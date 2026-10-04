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
# System services (PID 1..N, started by init); ahci/usb_storage drivers exist only when the controller is present.
SERVICES = ("init", "rtc", "ps2_kbd", "compositor", "ata", "ahci", "usb_storage", "vfs_server", "loader", "audio_gw", "tts", "shell")
# Test suites number apps from 1; the harness maps their numbers to real PIDs (BASE is computed at boot).
BASE = 0
PID_IN = re.compile(r"\b(fg|kill|logs)(\s+)(\d{1,18})\b", re.I)
PID_OUT = re.compile(r"(PID[= ])(\d+)")


def to_real(text):
    return PID_IN.sub(lambda m: f"{m[1]}{m[2]}{int(m[3]) + BASE if int(m[3]) > 0 else m[3]}", text)


def to_ordinal(text):
    return PID_OUT.sub(lambda m: f"{m[1]}{int(m[2]) - BASE}" if int(m[2]) > BASE else m[0], text)


class VM:
    def __init__(self, args, disk, usb=False, rtc="localtime", audio=None, ahci=False, prompt=True):
        self.disk = disk
        self.cpus = args.cpus
        filename = disk.replace(",", ",,")
        storage = (["-drive", f"format=raw,file={filename},if=none,id=usbdisk",
                    "-device", "qemu-xhci", "-device", "usb-storage,drive=usbdisk,bootindex=1"]
                   if usb else ["-drive", f"format=raw,file=fat:{filename},if=none,id=sata",
                                "-device", "ahci,id=ahci", "-device", "ide-hd,drive=sata,bus=ahci.0"]
                   if ahci else ["-drive", f"format=raw,file=fat:{filename}"])
        self.process = subprocess.Popen(
            [args.qemu, "-bios", args.firmware, *storage,
             "-snapshot", "-m", "512", "-smp", f"{args.cpus},sockets=1,cores={args.cpus},threads=1",
             "-serial", "mon:stdio", "-display", "none", "-rtc", f"base={rtc}", "-no-reboot",
             *(["-audiodev", "none,id=snd0" if audio == "none" else f"wav,id=snd0,path={audio}", "-device", "AC97,audiodev=snd0"] if audio else [])],
            cwd=ROOT, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
        )
        self.queue = queue.Queue()
        self.output = ""
        self.log = ""
        self.monitor = False
        threading.Thread(target=self._read, daemon=True).start()
        if not prompt:
            return
        try:
            self.expect("MIND> ", timeout=30)
            global BASE
            BASE = len(self.services())
        except BaseException:
            self.close()
            raise

    def services(self):
        # Real service PIDs from the ps table (the harness does not translate its rows).
        return {name: int(pid) for pid, name in re.findall(r"^(\d+) ([\w-]+) ", self.command("ps", raw=True), re.M) if name in SERVICES}

    def service_logs(self, name, until=None):
        # Service log; with `until`, wait for the line (drivers initialize in parallel with the test).
        output = ""
        for _ in range(40):
            output += self.command(f"logs {self.services()[name]}", raw=True)
            if until is None or until in output:
                break
            time.sleep(.25)
        return output

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
            clean = to_ordinal(ANSI.sub("", self.output).replace("\r", ""))
            if "KERNEL EXCEPTION" in clean or "KERNEL PANIC" in clean:
                raise AssertionError(clean)
            if text in clean:
                self.output = ""
                return clean
            if self.process.poll() is not None:
                raise AssertionError(f"QEMU exited {self.process.returncode}: {clean[-3000:]}")
            time.sleep(.01)
        raise AssertionError(f"Timeout waiting for {text!r}: {clean[-3000:]}")

    def send(self, text, raw=False):
        if not self.monitor and not raw:
            text = to_real(text)
        # Pace the UART, including Windows' line-buffered pipe input, rather than
        # overrunning the emulated 16550 FIFO with several pasted commands.
        for byte in text.encode("ascii"):
            self.process.stdin.write(bytes([byte]))
            self.process.stdin.flush()
            time.sleep(.01)

    def command(self, text, raw=False):
        self.send(text + "\n", raw)
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
        for char in to_real(text):
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
    # Apps only: services are visible in ps, but the suites check user tasks.
    return {int(m[0]) - BASE: m[1:] for m in re.findall(
        r"^(-?\d+) ([\w-]+) (READY|RUNNING|SLEEPING|EXITED|IPC_WAIT|IRQ_WAIT) (BG|FG) (\d+) (\d+) (\d+) (\d+)$", output, re.M)
        if m[1] not in SERVICES}


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
    # Idle: the CPU sleeps in HLT (one sample may catch it handling a tick, so a few are taken).
    for _ in range(10):
        registers = vm.hmp("info registers")
        if "HLT=1" in registers:
            break
        time.sleep(.02)
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
             ("y", 6, 0), ("e", "(?:6|13)", 0), ("h", 13, 0x102),
             # Read-only memory mint written to; a revoked lease read afterwards.
             ("m", 14, 7), ("v", 14, 4),
             # Address of a detached block.
             ("d", 14, 4),
             # Lease dropped after mapping, then revoked by the owner.
             ("l", 14, 4)]
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
    vm.send("run app2\n")
    vm.expect("RING3 IOPL0 READY")
    vm.send("k\n")
    output = vm.expect(f"PID={len(cases) + 3} EXITED. SHELL RESUMED.")
    require(output, "CAPABILITY CHECKS OK")
    time.sleep(.1); vm.collect(); vm.output = ""
    # A send queued for a server that dies fails with ERR_PEER instead of waiting for a new instance.
    parent = len(cases) + 4
    vm.send("run app2\n")
    vm.expect("RING3 IOPL0 READY")
    vm.send("f\n")
    output = vm.expect(f"PID={parent} EXITED. SHELL RESUMED.")
    require(output, "PARENT EXITS")
    time.sleep(.5); vm.collect(); vm.output = ""
    assert parent + 1 not in task_rows(vm), "the child must not wait for a future instance"
    assert f"FAULT PID={parent + 1} " not in vm.command("faults")

    # Cases with children started through loader (they get the parent's endpoint in their INIT slot).
    def family(key, children, done):
        vm.send("run app2\n")
        pid = int(re.search(r"STARTED PID=(\d+) NAME=app2", vm.expect("RING3 IOPL0 READY"))[1])
        vm.send(key + "\n")
        require(vm.expect(f"PID={pid} EXITED. SHELL RESUMED.", timeout=20), done)
        time.sleep(.5); vm.collect(); vm.output = ""
        rows, faults = task_rows(vm), vm.command("faults")
        assert not any(pid + n in rows for n in range(1, children + 1)), (key, rows)
        return pid, faults
    for key, children, done in [("q", 5, "QUEUE BOUND OK"), ("j", 1, "LATE REPLY OK"), ("z", 1, "MOVE OK"), ("b", 1, "REVOKE PENDING OK")]:
        pid, faults = family(key, children, done)
        assert not any(f"FAULT PID={pid + n} " in faults for n in range(children + 1)), (key, faults)
    # The child keeps reading a lease when the parent revokes it: its next access faults (CAP_REVOKE waits for its CPU).
    pid, faults = family("x", 1, "LEASE REVOKED")
    assert re.search(fr"FAULT PID={pid + 1} CPU=\d+ VECTOR=14 ", faults), faults
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


def files_check(vm, pid):
    output = ""
    for _ in range(50):
        output += vm.command(f"logs {pid}")
        if "[FILES] DONE" in output:
            break
        time.sleep(.2)
    require(output, "[FILES] kernel.elf ")
    require(output, "<DIR>")
    size = (ROOT / "usb_root/kernel.elf").stat().st_size
    require(output, f"READ kernel.elf {size}/{size} BYTES MAGIC=7F454C46")
    efi = (ROOT / "usb_root/EFI/BOOT/BOOTX64.EFI").stat().st_size
    require(output, f"READ EFI/BOOT/BOOTX64.EFI {efi}/{efi} BYTES MAGIC=4D5A")
    return output


def ahci_suite(vm):
    baseline = heap_used(vm)
    services = vm.services()
    assert "ahci" in services and "usb_storage" not in services, services
    require(vm.service_logs("ata", "[ATA] NO DISK"), "[ATA] NO DISK")
    require(vm.service_logs("ahci", "[AHCI] PORT 0: "), "[AHCI] PORT 0: ")
    require(vm.service_logs("vfs_server", "[VFS] MOUNTED FAT16 FROM AHCI"), "[VFS] MOUNTED FAT16 FROM AHCI")
    require(vm.command("run files &"), "PID=1 NAME=files BACKGROUND")
    files_check(vm, 1)
    vm.command("kill 1")
    assert heap_used(vm) == baseline
    # A killed DMA driver is restarted only after its device was quiesced and its DMA region cleared (MC-6.3);
    # the VFS keeps reading through the same endpoint.
    require(vm.command(f"kill {services['ahci']}", raw=True), "KILLED PID=")
    log = vm.service_logs("init", "ahci RESTARTED")
    assert log.index("ahci DEVICE QUIESCED") < log.index("ahci RESTARTED"), log
    require(vm.service_logs("ahci", "[AHCI] PORT 0: "), "[AHCI] PORT 0: ")
    files = int(re.search(r"PID=(\d+) NAME=files BACKGROUND", vm.command("run files &"))[1])  # the restart took a PID
    files_check(vm, files)
    vm.command(f"kill {files}")
    print("PASS: AHCI driver in ring 3 (MMIO + DMA capabilities), VFS mounted from SATA, file reads, restart after device quiesce", flush=True)


def services_suite(vm):
    output = vm.command("ps")
    for name in ("rtc", "ps2_kbd", "compositor", "ata", "vfs_server", "loader", "audio_gw", "tts"):
        assert re.search(fr"^\d+ {name} (IPC_WAIT|IRQ_WAIT|SLEEPING|READY|RUNNING) BG", output, re.M), (name, output)
    # Monotonic clock: calibrated TSC with sub-millisecond resolution, never going backwards.
    clocks = [re.search(r"MONOTONIC NS=(\d+) RESOLUTION NS=(\d+) TSC HZ=(\d+)", vm.command("clock")) for _ in range(2)]
    assert all(clocks), clocks
    (first, resolution, hz), (second, _, _) = [tuple(map(int, c.groups())) for c in clocks]
    assert second > first and 0 < resolution < 1_000_000 and hz > 1_000_000, (first, second, resolution, hz)
    require(vm.command("fg -4"), "ERROR:")  # the harness does not translate negative numbers
    vm.send("fg 0\n"); vm.expect("ERROR: EXPECTED ONE POSITIVE PID\nMIND> ")  # the whole reply, prompt included
    # End of the initial distribution: init gives up the platform privilege before READY.
    require(vm.service_logs("init", "[INIT] READY"), "[INIT] PLATFORM PRIVILEGE DROPPED")
    # Quotas delegated at spawn: init holds the root quota, loader may run 8 applications with 4 endpoints each.
    quotas = vm.command("quotas", raw=True)
    assert re.search(r"^\d+ loader 0/8 0/32$", quotas, re.M) and re.search(r"^1 init \d+/31 \d+/127$", quotas, re.M), quotas
    # Services do not occupy a screen and are not restarted.
    require(vm.command("run rtc &"), "SERVICE ALREADY RUNNING")
    baseline = heap_used(vm)
    # IPC: pong launches ping, receives a memory capability and replies to CALL.
    require(vm.command("run pong &"), "PID=1 NAME=pong BACKGROUND")
    time.sleep(3.5)
    pong = vm.command("logs 1")
    require(pong, "SPAWNED PING PID=2")
    require(pong, "FROM PID 2: HELLO FROM PING! ZERO-COPY IPC SUCCESS! COUNT: 1001")
    require(vm.command("logs 2"), "[PING] ACK 1001")
    # Killing a server whose client awaits a reply wakes the client with an error instead of hanging the kernel.
    require(vm.command("kill 1"), "KILLED PID=1")
    time.sleep(.5)
    assert 2 in task_rows(vm), "client of a dead server must survive"
    require(vm.command("kill 2"), "KILLED PID=2")
    # VFS: list the root and read files from the ATA disk via ata -> vfs_server.
    require(vm.service_logs("vfs_server", "[VFS] MOUNTED FAT16 FROM ATA"), "[VFS] MOUNTED FAT16 FROM ATA")
    require(vm.command("run files &"), "PID=3 NAME=files BACKGROUND")
    files_check(vm, 3)
    require(vm.command("kill 3"), "KILLED PID=3")
    # loader: programs are read from disk, not the kernel table — new files launch too.
    listing = vm.command("list")
    for name in ("clock", "dzen-clock", "hello", "files"):
        require(listing, f"  {name} ")
    assert "kernel " not in listing
    require(vm.command("run hello &"), "PID=4 NAME=hello BACKGROUND")
    require(vm.command("run extra/demo.elf &"), "PID=5 NAME=demo BACKGROUND")
    time.sleep(1.2)
    require(vm.command("logs 4"), "[CLOCK] ")
    assert {4, 5} <= set(task_rows(vm)), task_rows(vm)
    for text, error in [("run kernel", "UNKNOWN PROGRAM"), ("run nothing", "UNKNOWN PROGRAM"), ("run extra", "ERROR:"),
                        ("run averyveryverylongname", "PROGRAM NAME TOO LONG")]:
        require(vm.command(text), error)
    vm.command("kill 4"); vm.command("kill 5")
    for _ in range(20):
        if heap_used(vm) == baseline:
            break
        time.sleep(.1)
    assert heap_used(vm) == baseline, "IPC/VFS clients leaked memory or shared mappings"
    # Supervision: init restarts a killed service from the endpoint it keeps, so a client started before the failure
    # reaches the new instance; after three restarts in 60 s the service is quarantined until the operator runs it.
    require(vm.command("run hello &"), "PID=6 NAME=hello BACKGROUND")

    def clock_resumes():
        vm.command("logs 6")
        output = ""
        for _ in range(30):
            output += vm.command("logs 6")
            if "[CLOCK] " in output:
                return
            time.sleep(.2)
        raise AssertionError("client did not reach the restarted rtc: " + output)

    def kill_rtc():
        pid = vm.services()["rtc"]
        require(vm.command(f"kill {pid}", raw=True), "KILLED PID=")
        for _ in range(30):
            if vm.services().get("rtc", pid) != pid:
                return pid
            time.sleep(.1)
        return pid

    first = kill_rtc()
    require(vm.service_logs("init", f"rtc PID={first} KILLED"), "RESTARTED PID=")
    clock_resumes()
    kill_rtc(); kill_rtc(); kill_rtc()
    require(vm.service_logs("init", "QUARANTINED"), "rtc QUARANTINED: 3 RESTARTS IN 60 S")
    assert "rtc" not in vm.services(), "a quarantined service stays down"
    require(vm.command("run rtc &"), "NAME=rtc")
    clock_resumes()
    vm.command("kill 6")
    assert "FAULT PID=" not in vm.command("faults")
    # Final recovery boundary: without init the system stops instead of running unsupervised.
    vm.send(f"kill {vm.services()['init']}\n", raw=True)
    vm.expect("INIT EXITED: SYSTEM HALTED")
    print("PASS: boot services, monotonic clock, single instances, IPC call/reply with memory caps, peer death, VFS list/read over ATA driver + FAT, programs loaded from disk by loader, supervised restart with budget and quarantine for existing clients, halt without init, reclaim", flush=True)


def audio_suite(vm, wav):
    require(vm.command("run beep &"), "PID=1 NAME=beep BACKGROUND")
    output = ""
    for _ in range(50):
        output += vm.command("logs 1")
        if "[BEEP] DONE" in output:
            break
        time.sleep(.2)
    require(output, "[BEEP] DEVICE=true RATE=48000")
    require(output, "[BEEP] PCM QUEUED 24000 FRAMES")
    time.sleep(1.5)
    vm.command("kill 1")
    vm.close()
    import struct, wave
    with wave.open(str(wav)) as audio:
        frames = audio.readframes(audio.getnframes())
        rate = audio.getframerate()
    left = struct.unpack(f"<{len(frames) // 2}h", frames)[0::2]
    loud = [i for i, sample in enumerate(left) if sample]
    assert loud, "AC97 produced no audio"
    seconds = (loud[-1] - loud[0]) / rate
    assert 0.8 < seconds < 1.3, seconds  # 3 tones of 150 ms + 0.5 s sweep

    def power(start, hz):
        window = left[start:start + int(rate * 0.04)]
        return abs(sum(x * complex(math.cos(2 * math.pi * hz * i / rate), -math.sin(2 * math.pi * hz * i / rate))
                       for i, x in enumerate(window))) / len(window)
    for index, hz in enumerate((523, 659, 784)):
        start = loud[0] + int(rate * (0.05 + 0.15 * index))
        assert power(start, hz) > 5 * max(power(start, other) for other in (523, 659, 784) if other != hz), hz
    print("PASS: audio gateway: AC97 DMA ring, IRQ via IPC, tones 523/659/784 Hz and client PCM in captured audio", flush=True)


def listen_suite(vm):
    # QEMU's "none" backend feeds the AC97 microphone with silence at the real rate ("wav" has no capture).
    require(vm.command("run listen 1 &"), "PID=1 NAME=listen BACKGROUND")
    log = ""
    for _ in range(40):
        log += vm.command("logs 1")
        if "[LISTEN] DONE" in log:
            break
        time.sleep(.25)
    require(log, "[LISTEN] RECORDING 1 S")
    require(log, "[LISTEN] RECORDED 48000 FRAMES (1000 MS), PEAK 0, RMS 0, OVERFLOWS 0")
    require(log, "[LISTEN] PLAYED BACK")
    vm.command("kill 1")
    # Program arguments: options and text reach say; a plain word runs a program in the foreground.
    vm.send("run say -p 150 -r 120 hello world\n")
    output = vm.expect("PID=2 EXITED. SHELL RESUMED.", timeout=20)
    require(output, "STARTED PID=2 NAME=say FOREGROUND")
    # Two short words, not the default greeting (2.5-9 s): the text argument reached say.
    spoken = int(re.search(r"\[SAY\] SPOKE (\d+) MS", output)[1])
    assert 200 < spoken < 2000, spoken
    # Run by name: a plain word starts the program in the foreground with the rest as arguments.
    vm.send("say hi\n")
    output = vm.expect("PID=3 EXITED. SHELL RESUMED.", timeout=20)
    require(output, "STARTED PID=3 NAME=say FOREGROUND")
    spoken = int(re.search(r"\[SAY\] SPOKE (\d+) MS", output)[1])
    assert 50 < spoken < 1500, spoken
    require(vm.command("nosuchprogram"), "ERROR: UNKNOWN COMMAND")
    require(vm.command("run rtc x &"), "SERVICES TAKE NO ARGUMENTS")
    assert "FAULT PID=" not in vm.command("faults")
    print("PASS: microphone capture through audio_gw (48 kHz, AC97 PCM in), playback, program arguments, run by name", flush=True)


def tts_suite(vm, wav, asr_model=None):
    require(vm.service_logs("tts", "[TTS] FORMANT SYNTHESIZER READY"), "AUDIO=true")
    require(vm.command("run say &"), "PID=1 NAME=say BACKGROUND")
    output = ""
    for _ in range(80):
        output += vm.command("logs 1")
        if "[SAY] DONE" in output:
            break
        time.sleep(.25)
    spoken = int(re.search(r"\[SAY\] SPOKE (\d+) MS", output)[1])
    assert 2500 < spoken < 9000, spoken
    time.sleep(spoken / 1000 + 1)  # wait for DMA ring playback to finish
    vm.command("kill 1")
    vm.close()
    import struct, wave
    with wave.open(str(wav)) as audio:
        frames, rate = audio.readframes(audio.getnframes()), audio.getframerate()
    left = struct.unpack(f"<{len(frames) // 2}h", frames)[0::2]
    loud = [i for i, sample in enumerate(left) if abs(sample) > 300]
    assert loud, "TTS produced no audio"
    seconds = (loud[-1] - loud[0]) / rate
    assert abs(seconds * 1000 - spoken) < 1500, (seconds, spoken)
    # Voice: fundamental pitch in energetic windows — autocorrelation in the 80–160 Hz range.
    pitches = []
    for start in range(loud[0], loud[-1] - 2048, rate // 10):
        window = left[start:start + 2048]
        if sum(x * x for x in window) / len(window) < 1e6:
            continue
        lags = range(rate // 400, rate // 60)
        best = max(lags, key=lambda lag: sum(window[i] * window[i + lag] for i in range(0, 2048 - lag, 4)))
        pitches.append(rate / best)
    voiced = [p for p in pitches if 80 <= p <= 160]
    assert len(voiced) > len(pitches) / 2, pitches
    if asr_model:
        import json, vosk
        vosk.SetLogLevel(-1)
        step = rate / 16000
        mono = [left[int(i * step)] for i in range(int(len(left) / step))]
        recognizer = vosk.KaldiRecognizer(vosk.Model(asr_model), 16000)
        recognizer.AcceptWaveform(struct.pack(f"<{len(mono)}h", *mono))
        heard = json.loads(recognizer.FinalResult())["text"]
        print(f"ASR: {heard!r}", flush=True)
        assert sum(word in heard for word in ("разум", "корабля", "система", "работе")) >= 3, heard
    print(f"PASS: text to speech in ring 3: {spoken} ms of speech through audio_gw, voiced pitch {sum(voiced) / len(voiced):.0f} Hz" + (", recognized by ASR" if asr_model else ""), flush=True)


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


def boot_suite(args, disk):
    # The bootloader names a broken or missing boot file instead of hanging silently.
    kernel = (disk / "kernel.elf").read_bytes()
    for name, data, reason in [("kernel.elf", b"XELF" + kernel[4:], "bad ELF magic"),
                               ("kernel.elf", kernel[:40], "file too short for an ELF header"),
                               ("kernel.elf", kernel[:64] + b"\0" * 64, "program headers outside the file"),
                               ("rtc.elf", None, "file not found")]:
        target = disk / name
        original = target.read_bytes()
        if data is None:
            target.unlink()
        else:
            target.write_bytes(data)
        vm = VM(args, disk.relative_to(ROOT).as_posix(), prompt=False)
        try:
            vm.expect(f"BOOT ERROR: {name}: {reason}", timeout=30)
        finally:
            vm.close()
        target.write_bytes(original)
    print("PASS: bootloader reports a corrupt kernel ELF (magic, truncated header, program headers) and a missing boot file", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--qemu", default=os.environ.get("QEMU", "qemu-system-x86_64"))
    parser.add_argument("--cpus", type=int, default=4)
    parser.add_argument("--firmware", default="OVMF.fd")
    parser.add_argument("--busy-elf", help="test-only ELF built from tests/busy_app.rs")
    parser.add_argument("--isolation-elf", help="test-only ELF built from tests/isolation_app.rs")
    parser.add_argument("--heap-elf", help="test-only ELF built from tests/heap_app.rs")
    parser.add_argument("--suites", help="comma-separated subset: boot,normal,memory,dzen,services,ahci,audio,tts,listen,busy,smp,isolation,heap")
    parser.add_argument("--asr-model", help="optional Vosk model directory (Russian) to check that tts speech is recognizable")
    args = parser.parse_args()
    suites = ["boot", "normal", "memory", "dzen", "services", "ahci", "audio", "tts", "listen"] + (["busy", "smp"] if args.busy_elf else [])
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
            for name in [*(p.name for p in (ROOT / "usb_root").glob("*.elf")), "EFI/BOOT/BOOTX64.EFI"]:
                shutil.copyfile(ROOT / "usb_root" / name, disk / name)
            if suite == "services":
                # Files the kernel and ABI know nothing about: only loader will find them.
                shutil.copyfile(disk / "clock.elf", disk / "hello.elf")
                (disk / "extra").mkdir()
                shutil.copyfile(disk / "app.elf", disk / "extra/demo.elf")
            if suite in ("busy", "smp"):
                shutil.copyfile(args.busy_elf, disk / "app2.elf")
            elif suite == "isolation":
                shutil.copyfile(args.isolation_elf, disk / "app2.elf")
            elif suite == "heap":
                shutil.copyfile(args.heap_elf, disk / "app2.elf")
            elif suite == "memory":
                large_bss(disk / "app2.elf")
            if suite == "boot":
                boot_suite(args, disk)
                continue
            wav = Path(tempfile.gettempdir()) / f"mind-core-{suite}.wav" if suite in ("audio", "tts") else "none" if suite == "listen" else None
            vm = VM(args, disk.relative_to(ROOT).as_posix(),
                    rtc="2026-09-19T19:35:05" if suite == "dzen" else "localtime", audio=wav, ahci=suite == "ahci")
            try:
                if suite == "audio":
                    audio_suite(vm, wav)
                elif suite == "tts":
                    tts_suite(vm, wav, args.asr_model)
                elif suite == "listen":
                    listen_suite(vm)
                else:
                    {"normal": normal_suite, "busy": busy_suite, "memory": memory_suite,
                     "smp": smp_suite, "isolation": isolation_suite, "heap": heap_suite,
                     "dzen": dzen_suite, "services": services_suite, "ahci": ahci_suite}[suite](vm)
            finally:
                vm.close()
                log = Path(tempfile.gettempdir()) / f"mind-core-{suite}-{args.cpus}cpu.log"
                log.write_text(vm.log)
                print(f"QEMU log: {log}", flush=True)


if __name__ == "__main__":
    main()
