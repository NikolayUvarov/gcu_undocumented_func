#!/usr/bin/env python3
"""Exercise the real bootloader/kernel/apps via QEMU's UART and HMP.

Uses only Python's standard library. QEMU may be a native binary or Windows QEMU
from WSL. Run after 02_build.sh; pass --qemu and --firmware as needed. Temporary
FAT roots are created below usb_root and removed, leaving the built OS intact.
"""
import argparse
import math
import os
from pathlib import Path
import queue
import re
import shutil
import subprocess
import tempfile
import threading
import time

ROOT = Path(__file__).resolve().parents[1]
ANSI = re.compile(r"\x1b\[[0-9;?=]*[A-Za-z]")


class VM:
    def __init__(self, args, disk, usb=False, rtc="localtime"):
        self.disk = disk
        self.cpus = args.cpus
        filename = disk.replace(",", ",,")
        storage = (["-drive", f"format=raw,file={filename},if=none,id=usbdisk",
                    "-device", "qemu-xhci", "-device", "usb-storage,drive=usbdisk,bootindex=1"]
                   if usb else ["-drive", f"format=raw,file=fat:{filename}"])
        self.process = subprocess.Popen(
            [args.qemu, "-bios", args.firmware, *storage,
             "-snapshot", "-m", "512", "-smp", f"{args.cpus},sockets=1,cores={args.cpus},threads=1",
             "-serial", "mon:stdio", "-display", "none", "-rtc", f"base={rtc}", "-no-reboot"],
            cwd=ROOT, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
        )
        self.queue = queue.Queue()
        self.output = ""
        self.log = ""
        self.monitor = False
        threading.Thread(target=self._read, daemon=True).start()
        try:
            self.expect("MIND> ", timeout=30)
        except BaseException:
            self.close()
            raise

    def _read(self):
        while data := self.process.stdout.read(1):
            self.queue.put(data.decode("ascii", errors="replace"))

    def collect(self):
        while True:
            try:
                char = self.queue.get_nowait()
                self.output += char
                self.log += char
            except queue.Empty:
                return

    def expect(self, text, timeout=8):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            self.collect()
            clean = ANSI.sub("", self.output).replace("\r", "")
            if "KERNEL EXCEPTION" in clean or "KERNEL PANIC" in clean:
                raise AssertionError(clean)
            if text in clean:
                self.output = ""
                return clean
            if self.process.poll() is not None:
                raise AssertionError(f"QEMU exited {self.process.returncode}: {clean[-3000:]}")
            time.sleep(.01)
        raise AssertionError(f"Timeout waiting for {text!r}: {clean[-3000:]}")

    def send(self, text):
        # Pace the UART, including Windows' line-buffered pipe input, rather than
        # overrunning the emulated 16550 FIFO with several pasted commands.
        for byte in text.encode("ascii"):
            self.process.stdin.write(bytes([byte]))
            self.process.stdin.flush()
            time.sleep(.01)

    def command(self, text):
        self.send(text + "\n")
        return self.expect("MIND> ")

    def hmp(self, command):
        if not self.monitor:
            self.send("\x01c\n")
            self.expect("(qemu)")
            self.monitor = True
        self.send(command + "\n")
        return self.expect("(qemu)")

    def serial(self):
        if self.monitor:
            self.send("\x01c\n")
            self.monitor = False
            time.sleep(.1)
            self.collect()
            self.output = ""

    def background(self, pid):
        self.send("\x1a\n")
        self.expect(f"PID={pid} BACKGROUND. SHELL RESUMED.")
        # The LF used to flush Windows stdin may also create an empty command.
        time.sleep(.1)
        self.collect()
        self.output = ""

    def keys(self, text):
        for char in text:
            key = {" ": "spc", "\n": "ret", "&": "shift-7"}.get(char, char)
            self.hmp(f"sendkey {key} 30")
            time.sleep(.06)

    def screenshot(self):
        path = ROOT / (self.disk + ".ppm")
        try:
            self.hmp(f"screendump {path.relative_to(ROOT).as_posix()}")
            data = path.read_bytes()
            # Retain a viewable artifact outside the build tree.
            (Path(tempfile.gettempdir()) / "mind-core-clock.ppm").write_bytes(data)
            return data
        finally:
            path.unlink(missing_ok=True)

    def close(self):
        if self.process.poll() is None:
            self.process.terminate()
            self.process.wait(timeout=10)
        self.collect()


def require(text, fragment):
    assert fragment in text, (fragment, text)


def heap_used(vm):
    output = vm.command("heap")
    require(output, "TEST FREED=true")
    return int(re.search(r"HEAP: USED=(\d+)", output).group(1))


def task_rows(vm):
    output = vm.command("ps")
    return {int(m[0]): m[1:] for m in re.findall(
        r"^(\d+) ([\w-]+) (READY|RUNNING|SLEEPING|EXITED) (BG|FG) (\d+) (\d+) (\d+) (\d+)$", output, re.M)}


def center_pixel(vm):
    _, size, _, pixels = vm.screenshot().split(b"\n", 3)
    width, height = map(int, size.split())
    at = ((height // 2) * width + width // 2) * 3
    return pixels[at:at + 3]


def normal_suite(vm):
    baseline = heap_used(vm)
    require(vm.command("list"), "clock")
    require(vm.command("run clock &"), "PID=1 NAME=clock BACKGROUND")
    require(vm.command("run clock &"), "PID=2 NAME=clock BACKGROUND")
    require(vm.command("run app &"), "PID=3 NAME=app BACKGROUND")
    require(vm.command("run app &"), "PID=4 NAME=app BACKGROUND")
    require(vm.command("run app2 &"), "PID=5 NAME=app2 BACKGROUND")
    first = task_rows(vm)
    time.sleep(.4)
    second = task_rows(vm)
    assert set(second) == {1, 2, 3, 4, 5}, second
    assert all(int(second[p][-1]) > int(first[p][-1]) for p in first), (first, second)
    require(vm.command("logs 1"), "[CLOCK]")
    app2_log = vm.command("logs 5")
    require(app2_log, "FRAME=0")
    require(app2_log, "PRIVATE HEAP SPRITE READY")
    for text in ["fg", "fg 0", "fg -1", "fg 1 2", "fg 999", "kill 999", "kill 0",
                 "fg 9999999999999999999999999", "run missing"]:
        require(vm.command(text), "ERROR:")
    require(vm.command("run"), "USAGE:")
    vm.send("fg 1\n")
    vm.expect("FOREGROUND PID=1")
    vm.background(1)
    assert 1 in task_rows(vm)
    vm.send("fg 2\n")
    vm.expect("FOREGROUND PID=2")
    vm.send("\x1b\n")
    vm.expect("PID=2 EXITED. SHELL RESUMED.")
    time.sleep(.1)
    vm.collect(); vm.output = ""
    assert set(task_rows(vm)) == {1, 3, 4, 5}
    # Changing the foreground square's color must not affect the other copy.
    vm.send("fg 3\n")
    vm.expect("FOREGROUND PID=3")
    vm.send("g\n")
    time.sleep(.15)
    assert center_pixel(vm) == b"\x00\xff\x00", "input must reach PID 3"
    vm.serial(); vm.background(3)
    vm.send("fg 4\n")
    vm.expect("FOREGROUND PID=4")
    assert center_pixel(vm) == b"\x00\xff\xff", "PID 4 must keep its own color"
    vm.serial(); vm.background(4)
    vm.send("fg 3\n")
    vm.expect("FOREGROUND PID=3")
    assert center_pixel(vm) == b"\x00\xff\x00", "fg must preserve state, not restart"
    vm.serial(); vm.background(3)
    require(vm.command("kill 3"), "KILLED PID=3")
    assert set(task_rows(vm)) == {1, 4, 5}
    vm.send("fg 4\n")
    vm.expect("FOREGROUND PID=4")
    # Exercise the physical Ctrl+Z keyboard route, not just UART.
    vm.hmp("sendkey ctrl-z")
    vm.serial()
    assert 4 in task_rows(vm)
    vm.send("fg 5\n")
    vm.expect("FOREGROUND PID=5")
    vm.send("\x1b\n")
    vm.expect("PID=5 EXITED. SHELL RESUMED.")
    time.sleep(.1); vm.collect(); vm.output = ""
    for pid in [1, 4]:
        require(vm.command(f"kill {pid}"), f"KILLED PID={pid}")
    assert heap_used(vm) == baseline, "task teardown leaked resources"
    for pid in range(6, 14):
        require(vm.command("run clock &"), f"PID={pid} NAME=clock BACKGROUND")
    require(vm.command("run app &"), "TASK LIMIT REACHED")
    assert len(task_rows(vm)) == 8
    for pid in range(6, 14):
        vm.command(f"kill {pid}")
    assert heap_used(vm) == baseline, "slot exhaustion/reuse leaked resources"
    # Repeated creation/freeing must retain the same empty-system heap baseline.
    for pid in range(14, 24):
        require(vm.command("run app &"), f"PID={pid} NAME=app BACKGROUND")
        vm.command(f"kill {pid}")
    assert heap_used(vm) == baseline
    vm.send("boot\n")
    vm.expect("PID=24 NAME=app FOREGROUND")
    vm.hmp("sendkey esc")
    vm.serial()
    assert task_rows(vm) == {}
    vm.keys("run clock &\n")
    vm.serial()
    assert 25 in task_rows(vm), "PS/2 Shift+7 must produce a background launch"
    vm.keys("fg 25\n")
    # Give the clock a chance to render, then capture the actual foreground.
    time.sleep(.2)
    screenshot = vm.screenshot()
    magic, size, maximum, pixels = screenshot.split(b"\n", 3)
    width, height = map(int, size.split())
    assert magic == b"P6" and maximum == b"255"
    green = sum(pixels[(y * width + x) * 3:(y * width + x) * 3 + 3] == b"\xa6\xe3\xa1"
                for y in range(height // 3, 2 * height // 3)
                for x in range(width // 4, 3 * width // 4))
    assert green > 1000, "fg must display the actual clock framebuffer"
    vm.hmp("sendkey ctrl-z")
    vm.serial()
    assert 25 in task_rows(vm), "PS/2 fg/Ctrl+Z must preserve the task"
    vm.keys("kill 25\n")
    vm.serial()
    assert task_rows(vm) == {}
    registers = vm.hmp("info registers")
    require(registers, "HLT=1")
    cpus = vm.hmp("info cpus")
    assert len(re.findall(r"CPU #\d", cpus)) == vm.cpus, cpus
    vm.serial()
    assert heap_used(vm) == baseline
    print("PASS: instances, concurrent progress, fg, Ctrl+Z/UART+PS2, Esc, kill, logs, invalid input, limit/reuse, heap, HLT, 4 CPUs", flush=True)


def busy_suite(vm):
    baseline = heap_used(vm)
    require(vm.command("run app2 &"), "PID=1 NAME=app2 BACKGROUND")
    require(vm.command("run clock &"), "PID=2 NAME=clock BACKGROUND")
    first = task_rows(vm)
    time.sleep(.4)
    second = task_rows(vm)
    assert int(second[1][-2]) > int(first[1][-2]), (first, second)
    assert int(second[1][-1]) == 1, second  # only the initial UART syscall
    assert int(second[2][-1]) > int(first[2][-1]), (first, second)
    require(vm.command("logs 1"), "BUSY FIXTURE")
    require(vm.command("kill 1"), "KILLED PID=1")
    vm.command("kill 2")
    assert heap_used(vm) == baseline
    print("PASS: timer preemption of a non-yielding SIMD loop; responsive shell, clocks and kill", flush=True)


def smp_suite(vm):
    baseline = heap_used(vm)
    cpus = vm.command("cpus")
    assert len(re.findall(r"ONLINE=true", cpus)) == vm.cpus, cpus
    # Two non-yielding SIMD loops per core force real preemption on every CPU.
    count = min(vm.cpus * 2, 8)
    for pid in range(1, count + 1):
        require(vm.command("run app2 &"), f"PID={pid} NAME=app2 BACKGROUND")
    first = task_rows(vm)
    time.sleep(.5)
    second = task_rows(vm)
    assert {int(row[3]) for row in second.values()} == set(range(vm.cpus)), second
    assert all(int(second[p][-2]) > int(first[p][-2]) for p in second), (first, second)
    assert all(int(row[-1]) == 1 for row in second.values()), second
    assert "FAULT PID=" not in vm.command("faults")
    for pid in range(1, count + 1):
        require(vm.command(f"kill {pid}"), f"KILLED PID={pid}")
    assert heap_used(vm) == baseline
    for cpu in range(vm.cpus):
        vm.hmp(f"cpu {cpu}")
        for _ in range(10):
            regs = vm.hmp("info registers")
            if "HLT=1" in regs:
                break
            time.sleep(.02)
        else:
            raise AssertionError(f"CPU {cpu} did not halt when idle: {regs}")
    vm.serial()
    print(f"PASS: {vm.cpus} online CPUs, concurrent pinned tasks, SIMD preservation, remote kill, all CPUs HLT", flush=True)


def isolation_suite(vm):
    baseline = heap_used(vm)
    require(vm.command("run clock &"), "PID=1")
    before = task_rows(vm)[1]
    cases = [("r", 14, 5), ("w", 14, 7), ("t", 14, 7), ("n", 14, 21),
             ("c", 13, 0), ("o", 13, 0), ("u", 6, 0), ("g", 14, 4), ("s", 6, 0),
             # AMD forbids SYSENTER in long mode (#UD); Intel checks CS=0 (#GP).
             ("y", 6, 0), ("e", "(?:6|13)", 0), ("h", 13, 0x102)]
    for pid, (key, vector, error) in enumerate(cases, 2):
        vm.send("run app2\n")
        vm.expect("RING3 IOPL0 READY")
        vm.send(key + "\n")
        vm.expect(f"PID={pid} EXITED. SHELL RESUMED.")
        time.sleep(.1); vm.collect(); vm.output = ""
        faults = vm.command("faults")
        assert re.search(fr"FAULT PID={pid} CPU=\d+ VECTOR={vector} ERROR={error:#x} ", faults), faults
        assert set(task_rows(vm)) == {1}
    vm.send("run app2\n")
    vm.expect("RING3 IOPL0 READY")
    vm.send("p\n")
    output = vm.expect(f"PID={len(cases) + 2} EXITED. SHELL RESUMED.")
    require(output, "POINTER VALIDATION OK")
    time.sleep(.1); vm.collect(); vm.output = ""
    assert int(task_rows(vm)[1][-1]) > int(before[-1])
    vm.command("kill 1")
    assert heap_used(vm) == baseline, "fault teardown leaked task/page-table resources"
    require(vm.command("run app &"), "NAME=app BACKGROUND")
    print("PASS: CPL3/IOPL0; kernel read/write, RX code, NX stack, CLI/I/O, UD2, guard/bad stack; syscall pointers; fault containment and reclaim", flush=True)


def memory_suite(vm):
    baseline = heap_used(vm)
    pids = []
    for _ in range(8):
        before = heap_used(vm)
        output = vm.command("run app2 &")
        if "OUT OF MEMORY" in output:
            assert heap_used(vm) == before, "partial spawn must roll back all allocations"
            break
        match = re.search(r"STARTED PID=(\d+)", output)
        assert match, output
        pids.append(int(match[1]))
    else:
        raise AssertionError("large-BSS fixture did not exercise allocation failure")
    assert 1 < len(pids) < 8, pids
    first = task_rows(vm)
    time.sleep(.3)
    second = task_rows(vm)
    assert all(int(second[p][-1]) > int(first[p][-1]) for p in pids)
    for pid in pids:
        vm.command(f"kill {pid}")
    assert heap_used(vm) == baseline
    require(vm.command("run clock &"), "NAME=clock BACKGROUND")
    print("PASS: out-of-memory rollback, surviving tasks and later successful launch", flush=True)


def heap_suite(vm):
    baseline = heap_used(vm)
    require(vm.command("run clock &"), "PID=1")
    clock_baseline = heap_used(vm)
    next_pid = 2

    def start():
        nonlocal next_pid
        pid = next_pid
        next_pid += 1
        vm.send("run app2\n")
        vm.expect("HEAP READY")
        return pid

    def foreground(pid):
        vm.send(f"fg {pid}\n")
        vm.expect(f"FOREGROUND PID={pid}")

    def exited(pid):
        vm.expect(f"PID={pid} EXITED. SHELL RESUMED.")
        time.sleep(.1); vm.collect(); vm.output = ""

    pid = start()
    vm.background(pid)
    process_baseline = heap_used(vm)
    foreground(pid)
    vm.send("t\n")
    vm.expect("HEAP CHECKS OK", timeout=20)
    vm.background(pid)
    assert heap_used(vm) == process_baseline, "free must reclaim data AND page tables"
    foreground(pid)
    vm.send("e\n")
    exited(pid)
    assert heap_used(vm) == clock_baseline, "normal exit leaked outstanding allocations"

    # A TSC-derived private pattern is checked continuously at identical virtual
    # addresses while every CPU repeatedly allocates/frees other heap pages.
    stress_pids = []
    for _ in range(max(2, vm.cpus)):
        pid = start()
        vm.send("c\n")
        vm.expect("HEAP STRESS READY")
        vm.background(pid)
        stress_pids.append(pid)
    first = task_rows(vm)
    assert {int(first[p][3]) for p in stress_pids} == set(range(vm.cpus)), first
    for _ in range(10):
        heap_used(vm)  # concurrently exercises the IRQ-safe kernel allocator
    second = task_rows(vm)
    assert all(int(second[p][-1]) > int(first[p][-1]) for p in stress_pids), (first, second)
    assert "FAULT PID=" not in vm.command("faults")
    foreground(stress_pids[0])
    vm.send("e\n")
    exited(stress_pids[0])
    for pid in stress_pids[1:]:
        vm.command(f"kill {pid}")
    assert heap_used(vm) == clock_baseline, "remote kill leaked live private heaps"

    for key, error in [("n", 21), ("g", 6), ("u", 6)]:
        pid = start()
        vm.send(key + "\n")
        exited(pid)
        faults = vm.command("faults")
        assert re.search(fr"FAULT PID={pid} CPU=\d+ VECTOR=14 ERROR={error:#x} ", faults), faults
        assert set(task_rows(vm)) == {1}
        assert heap_used(vm) == clock_baseline, "fault teardown leaked heap"

    # Pre-create idle clients so the expected OOM occurs in ALLOC, not in RUN.
    clients = []
    for _ in range(4):
        pid = start()
        vm.background(pid)
        clients.append(pid)
    holders, failed = [], []
    for pid in clients:
        before = heap_used(vm)
        foreground(pid)
        vm.send("b\n")
        output = vm.expect("HEAP ALLOCATION FINISHED")
        vm.background(pid)
        if "OOM" in output:
            failed.append(pid)
            assert heap_used(vm) == before, "failed allocation changed heap usage"
        else:
            require(output, "QUOTA HELD")
            holders.append(pid)
    assert holders and failed, (holders, failed)
    vm.command(f"kill {holders[0]}")
    clients.remove(holders[0])
    foreground(failed[0])
    vm.send("b\n")
    require(vm.expect("HEAP ALLOCATION FINISHED"), "HEAP QUOTA HELD")
    vm.background(failed[0])
    for pid in clients:
        vm.command(f"kill {pid}")
    assert heap_used(vm) == clock_baseline
    assert int(task_rows(vm)[1][-1]) > int(first[1][-1])
    vm.command("kill 1")
    assert heap_used(vm) == baseline
    print(f"PASS: private heaps on {vm.cpus} CPUs; zero/reuse/limits/invalid free; page-table reclamation; concurrent allocator; NX/guards/stale TLB; OOM recovery; exit/kill/fault cleanup", flush=True)


def dzen_suite(vm):
    # The VM starts at 19:35:05: TR yellow, center cyan; clockwise free
    # corners BR dark, BL/TL white. There is >90 s before the next state.
    baseline = heap_used(vm)
    require(vm.command("list"), "dzen-clock")
    require(vm.command("run dzen-clock &"), "PID=1 NAME=dzen-clock BACKGROUND")
    require(vm.command("run dzen-clock &"), "PID=2 NAME=dzen-clock BACKGROUND")
    assert set(task_rows(vm)) == {1, 2}
    vm.send("fg 1\n")
    vm.expect("FOREGROUND PID=1")
    time.sleep(.2)

    def check_screen(digits, mode="off", hints=True):
        image = vm.screenshot()
        _, size, _, pixels = image.split(b"\n", 3)
        width, height = map(int, size.split())
        x, y = width // 2, height // 2 - 24
        half = min(min(width, height) // 5, 160)
        radius = max(half // 3, 1)
        def pixel(px, py):
            at = (py * width + px) * 3
            return pixels[at:at + 3]
        for px, py, color in [(x + half, y - half, b"\xff\xff\x00"),
                              (x + half, y + half, b"\x08\x0c\x12"),
                              (x - half, y + half, b"\x60\x60\x60"),
                              (x - half, y - half, b"\x60\x60\x60"),
                              (x, y, b"\x00\xff\xff")]:
            assert pixel(px, py) == color, (px, py, pixel(px, py), color)
        digital = [pixel(px, py) for py in range(y + half + radius + 28, y + half + radius + 49)
                   for px in range(x - 62, x + 62)]
        if digits:
            assert digital.count(b"\x69\x75\x83") > 80, "digital time must be visible"
        else:
            assert all(p == b"\x08\x0c\x12" for p in digital), "hidden digits must be fully erased"
        for top in (24, height - 32):
            text = [pixel(px, py) for py in range(top, top + 8) for px in range(width)]
            if hints:
                assert text.count(b"\x39\x45\x53") > 50, "title and key hints must be visible"
            else:
                assert all(p == b"\x08\x0c\x12" for p in text), "H must erase title and key hints"
        orbit = {(px - x, py - y): pixel(px, py)
                 for py in range(y - half, y + half + 1)
                 for px in range(x - half, x + half + 1)
                 if (radius + 6) ** 2 <= (px - x) ** 2 + (py - y) ** 2 <= (half - radius // 3) ** 2}
        assert all(p in (b"\x08\x0c\x12", b"\x80\x80\x80", b"\x18\x18\x18", b"\x28\x28\x28",
                         b"\x60\x60\x60", b"\x40\x40\x40") for p in orbit.values())
        mark = {point for point, color in orbit.items() if color == b"\x80\x80\x80"}
        if mode != "off":
            diameter = max(half // 24, 2)
            assert 0 < len(mark) < diameter ** 2, "a small round dot, not a square or stroke"
            for axis in (0, 1):
                assert max(p[axis] for p in mark) - min(p[axis] for p in mark) + 1 == diameter
            orbit_color = b"\x18\x18\x18" if mode == "simple" else b"\x28\x28\x28"
            other_color = b"\x28\x28\x28" if mode == "simple" else b"\x18\x18\x18"
            assert sum(p == orbit_color for p in orbit.values()) > half * 3, "orbit must use this mode's brightness"
            assert other_color not in orbit.values(), "switching modes must restore the correct orbit brightness"
            reference = {point for point, color in orbit.items() if color == b"\x60\x60\x60"}
            assert len(reference) >= 6, "start reference tick must remain"
            assert all(px > 0 and py > 0 and abs(px - py) <= 1 for px, py in reference), "cycle starts toward bottom right"
            small_ticks = sum(p == b"\x40\x40\x40" for p in orbit.values())
            if mode == "ticks":
                assert small_ticks >= 35, "nine short 10-second ticks must be visible"
                for index in range(1, 10):
                    angle = 3 * math.pi / 4 + index * math.tau / 10
                    tx, ty = int(math.sin(angle) * half * 3 / 4), -int(math.cos(angle) * half * 3 / 4)
                    assert any(orbit.get((tx + dx, ty + dy)) in (b"\x40\x40\x40", b"\x80\x80\x80")
                               for dy in range(-2, 3) for dx in range(-2, 3)), index
            else:
                assert small_ticks == 0, "simple orbit must have only the start tick"
            (Path(tempfile.gettempdir()) / "mind-core-dzen-cycle.ppm").write_bytes(image)
            (Path(tempfile.gettempdir()) / f"mind-core-dzen-{mode}.ppm").write_bytes(image)
            if not hints:
                (Path(tempfile.gettempdir()) / f"mind-core-dzen-{mode}-clean.ppm").write_bytes(image)
        else:
            assert all(p == b"\x08\x0c\x12" for p in orbit.values()), "hidden orbit must leave no dot/line/ticks"
        (Path(tempfile.gettempdir()) / f"mind-core-dzen-{'digits' if digits else 'quiet'}.ppm").write_bytes(image)
        vm.serial()
        return mark

    check_screen(True)
    vm.send("h\n")
    vm.expect("TEXT OFF")
    check_screen(True, hints=False)
    vm.send("c\n")
    vm.expect("ORBIT SIMPLE")
    first_mark = check_screen(True, "simple", hints=False)
    time.sleep(1.3)
    next_mark = check_screen(True, "simple", hints=False)
    assert first_mark != next_mark, "cycle mark must move while the face stays unchanged"
    ax, ay = (sum(p[i] for p in first_mark) for i in (0, 1))
    bx, by = (sum(p[i] for p in next_mark) for i in (0, 1))
    assert ax * by - ay * bx > 0, "cycle mark must move clockwise"
    vm.send("p\n")
    vm.expect("ORBIT 10S TICKS")
    check_screen(True, "ticks", hints=False)
    vm.send("d\n")
    vm.expect("DIGITS OFF")
    check_screen(False, "ticks", hints=False)
    vm.background(1)
    vm.send("fg 2\n")
    vm.expect("FOREGROUND PID=2")
    check_screen(True)
    vm.background(2)
    vm.send("fg 1\n")
    vm.expect("FOREGROUND PID=1")
    check_screen(False, "ticks", hints=False)
    vm.hmp("sendkey h 30")
    time.sleep(.2)
    check_screen(False, "ticks")
    vm.hmp("sendkey d 30")
    time.sleep(.2)
    check_screen(True, "ticks")
    vm.hmp("sendkey c 30")
    time.sleep(.2)
    check_screen(True, "simple")
    vm.hmp("sendkey p 30")
    time.sleep(.2)
    check_screen(True, "ticks")
    vm.send("C\n")
    vm.expect("ORBIT SIMPLE")
    check_screen(True, "simple")
    vm.send("c\n")
    vm.expect("ORBIT OFF")
    check_screen(True)
    vm.send("H\n")
    vm.expect("TEXT OFF")
    check_screen(True, hints=False)
    vm.send("h\n")
    vm.expect("TEXT ON")
    check_screen(True)
    vm.send("P\n")
    vm.expect("ORBIT 10S TICKS")
    check_screen(True, "ticks")
    vm.send("p\n")
    vm.expect("ORBIT OFF")
    check_screen(True)
    vm.send("\x1b\n")
    vm.expect("PID=1 EXITED. SHELL RESUMED.")
    time.sleep(.1); vm.collect(); vm.output = ""
    require(vm.command("kill 2"), "KILLED PID=2")
    assert heap_used(vm) == baseline
    assert "FAULT PID=" not in vm.command("faults")
    print("PASS: dzen-clock colors; small clockwise dot; darker C orbit; bottom-right start and 10s ticks; UART/PS2 C/P/D/H; clean title/hint toggle; mode switching and erasure; independent instances; fg/exit/reclaim", flush=True)


def large_bss(path):
    data = bytearray(path.read_bytes())
    offset = int.from_bytes(data[32:40], "little")
    count = int.from_bytes(data[56:58], "little")
    loads = [offset + i * 56 for i in range(count)
             if int.from_bytes(data[offset + i * 56:offset + i * 56 + 4], "little") == 1]
    last = loads[-1]
    size = int.from_bytes(data[last + 40:last + 48], "little") + 7 * 1024 * 1024
    data[last + 40:last + 48] = size.to_bytes(8, "little")
    path.write_bytes(data)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--qemu", default=os.environ.get("QEMU", "qemu-system-x86_64"))
    parser.add_argument("--cpus", type=int, default=4)
    parser.add_argument("--firmware", default="OVMF.fd")
    parser.add_argument("--busy-elf", help="test-only ELF built from tests/busy_app.rs")
    parser.add_argument("--isolation-elf", help="test-only ELF built from tests/isolation_app.rs")
    parser.add_argument("--heap-elf", help="test-only ELF built from tests/heap_app.rs")
    parser.add_argument("--suites", help="comma-separated subset: normal,memory,dzen,busy,smp,isolation,heap")
    args = parser.parse_args()
    suites = ["normal", "memory", "dzen"] + (["busy", "smp"] if args.busy_elf else [])
    if args.isolation_elf:
        suites.append("isolation")
    if args.heap_elf:
        suites.append("heap")
    if args.suites:
        suites = args.suites.split(",")
    for suite in suites:
        with tempfile.TemporaryDirectory(prefix="smoke-", dir=ROOT / "usb_root") as temp:
            disk = Path(temp)
            (disk / "EFI/BOOT").mkdir(parents=True)
            for name in ["kernel.elf", "app.elf", "app2.elf", "clock.elf", "dzenclk.elf", "EFI/BOOT/BOOTX64.EFI"]:
                shutil.copyfile(ROOT / "usb_root" / name, disk / name)
            if suite in ("busy", "smp"):
                shutil.copyfile(args.busy_elf, disk / "app2.elf")
            elif suite == "isolation":
                shutil.copyfile(args.isolation_elf, disk / "app2.elf")
            elif suite == "heap":
                shutil.copyfile(args.heap_elf, disk / "app2.elf")
            elif suite == "memory":
                large_bss(disk / "app2.elf")
            vm = VM(args, disk.relative_to(ROOT).as_posix(),
                    rtc="2026-09-19T19:35:05" if suite == "dzen" else "localtime")
            try:
                {"normal": normal_suite, "busy": busy_suite, "memory": memory_suite,
                 "smp": smp_suite, "isolation": isolation_suite, "heap": heap_suite,
                 "dzen": dzen_suite}[suite](vm)
            finally:
                vm.close()
                log = Path(tempfile.gettempdir()) / f"mind-core-{suite}-{args.cpus}cpu.log"
                log.write_text(vm.log)
                print(f"QEMU log: {log}", flush=True)


if __name__ == "__main__":
    main()
