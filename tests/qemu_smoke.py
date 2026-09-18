#!/usr/bin/env python3
"""Exercise the real bootloader/kernel/apps via QEMU's UART and HMP.

Uses only Python's standard library. QEMU may be a native binary or Windows QEMU
from WSL. Run after 02_build.sh; pass --qemu and --firmware as needed. Temporary
FAT roots are created below usb_root and removed, leaving the built OS intact.
"""
import argparse
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
    def __init__(self, args, disk):
        self.disk = disk
        self.process = subprocess.Popen(
            [args.qemu, "-bios", args.firmware, "-drive", f"format=raw,file=fat:{disk}",
             "-snapshot", "-m", "512", "-smp", "4,sockets=1,cores=4,threads=1",
             "-serial", "mon:stdio", "-display", "none", "-rtc", "base=localtime", "-no-reboot"],
            cwd=ROOT, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
        )
        self.queue = queue.Queue()
        self.output = ""
        self.log = ""
        self.monitor = False
        threading.Thread(target=self._read, daemon=True).start()
        self.expect("MIND> ", timeout=30)

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
        r"^(\d+) (\w+) (READY|SLEEPING|EXITED) (BG|FG) (\d+) (\d+) (\d+)$", output, re.M)}


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
    require(vm.command("logs 5"), "FRAME=0")
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
    for pid in [1, 4, 5]:
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
    assert len(re.findall(r"CPU #\d", cpus)) == 4, cpus
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
    parser.add_argument("--firmware", default="OVMF.fd")
    parser.add_argument("--busy-elf", help="test-only ELF built from tests/busy_app.rs")
    args = parser.parse_args()
    suites = ["normal", "memory"] + (["busy"] if args.busy_elf else [])
    for suite in suites:
        with tempfile.TemporaryDirectory(prefix="smoke-", dir=ROOT / "usb_root") as temp:
            disk = Path(temp)
            (disk / "EFI/BOOT").mkdir(parents=True)
            for name in ["kernel.elf", "app.elf", "app2.elf", "clock.elf", "EFI/BOOT/BOOTX64.EFI"]:
                shutil.copyfile(ROOT / "usb_root" / name, disk / name)
            if suite == "busy":
                shutil.copyfile(args.busy_elf, disk / "app2.elf")
            elif suite == "memory":
                large_bss(disk / "app2.elf")
            vm = VM(args, disk.relative_to(ROOT).as_posix())
            try:
                {"normal": normal_suite, "busy": busy_suite, "memory": memory_suite}[suite](vm)
            finally:
                vm.close()
                log = Path(tempfile.gettempdir()) / f"mind-core-{suite}.log"
                log.write_text(vm.log)
                print(f"QEMU log: {log}", flush=True)


if __name__ == "__main__":
    main()
