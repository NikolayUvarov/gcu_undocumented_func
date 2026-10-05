#!/usr/bin/env python3
"""Exercise the real bootloader/kernel/apps via QEMU's UART and HMP.

Uses only Python's standard library. QEMU may be a native binary or Windows QEMU
from WSL. Run after 02_build.sh; pass --qemu and --firmware as needed. Temporary
FAT roots are created below usb_root and removed, leaving the built OS intact.
"""
import argparse
import codecs
import http.server
import math
import json
import os
from pathlib import Path
import queue
import re
import shutil
import socket
import socketserver
import ssl
import struct
import subprocess
import tempfile
import threading
import time

ROOT = Path(__file__).resolve().parents[1]
ANSI = re.compile(r"\x1b\[[0-9;?=]*[A-Za-z]")
# System services (PID 1..N, started by init); ahci/usb_storage/virtio_net exist only when their device is present.
SERVICES = ("init", "logd", "rtc", "ps2_kbd", "compositor", "ata", "ahci", "usb_storage", "ramdisk", "vfs_server", "loader", "audio_gw", "tts", "virtio_net", "virtio_net#1", "netstack", "netpolicy", "keystore", "tls", "windows", "virtio_input", "sysmon", "shell")
# Test suites number apps from 1; the harness maps their numbers to real PIDs (BASE is computed at boot).
BASE = 0
PID_IN = re.compile(r"\b(fg|kill|logs|pmap|stat|caps|budget)(\s+)(\d{1,18})\b", re.I)
PID_OUT = re.compile(r"(PID[= ])(\d+)")


def to_real(text):
    return PID_IN.sub(lambda m: f"{m[1]}{m[2]}{int(m[3]) + BASE if int(m[3]) > 0 else m[3]}", text)


def to_ordinal(text):
    return PID_OUT.sub(lambda m: f"{m[1]}{int(m[2]) - BASE}" if int(m[2]) > BASE else m[0], text)


class VM:
    def __init__(self, args, disk, usb=False, rtc="localtime", audio=None, ahci=False, raw=False, snapshot=True, prompt=True, extra=(), reboot=False):
        # `disk` is a directory served as a virtual FAT disk, or with `raw` (always for USB) a disk image; without
        # `snapshot` writes reach the image.
        self.disk = disk
        self.cpus, self.args = args.cpus, args
        filename = disk.replace(",", ",,")
        source = f"format=raw,file={filename}" if usb or raw else f"format=raw,file=fat:{filename}"
        storage = (["-drive", f"{source},if=none,id=usbdisk",
                    "-device", "qemu-xhci", "-device", "usb-storage,drive=usbdisk,bootindex=1"]
                   if usb else ["-drive", f"{source},if=none,id=sata",
                                "-device", "ahci,id=ahci", "-device", "ide-hd,drive=sata,bus=ahci.0"]
                   if ahci else ["-drive", source])
        self.process = subprocess.Popen(
            [args.qemu, "-bios", args.firmware, *storage,
             *(["-snapshot"] if snapshot else []), "-m", "512", "-smp", f"{args.cpus},sockets=1,cores={args.cpus},threads=1",
             "-serial", "mon:stdio", "-display", "none", "-rtc", f"base={rtc}", *([] if reboot else ["-no-reboot"]), *extra,
             *(["-cpu", model] if (model := getattr(args, "cpu_model", None)) and "-cpu" not in extra else []),
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
        return {name: int(pid) for pid, name in re.findall(r"^(\d+) ([\w#-]+) ", self.command("ps", raw=True), re.M) if name in SERVICES}

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
        # The UART carries UTF-8 (Cyrillic in program output).
        decoder = codecs.getincrementaldecoder("utf-8")(errors="replace")
        while data := self.process.stdout.read(1):
            if text := decoder.decode(data):
                self.queue.put(text)

    def collect(self):
        while True:
            try:
                char = self.queue.get_nowait()
                self.output += char
                self.log += char
            except queue.Empty:
                return

    def expect(self, text, timeout=8, after=None):
        """Waits for `text` (after the first `after`, when given) in the output since the last match."""
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            self.collect()
            clean = to_ordinal(ANSI.sub("", self.output).replace("\r", ""))
            if "KERNEL EXCEPTION" in clean or "KERNEL PANIC" in clean:
                raise AssertionError(clean)
            if text in (clean if after is None else clean.partition(after)[2]):
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

    def send_bytes(self, data):
        for byte in data:
            self.process.stdin.write(bytes([byte]))
            self.process.stdin.flush()
            time.sleep(.01)

    def command(self, text, raw=False):
        self.send(text + "\n", raw)
        # The prompt after the echo of this line: a prompt printed late for the previous command is not this one's.
        return self.expect("MIND> ", after=to_ordinal(text if raw or self.monitor else to_real(text)) + "\n")

    def hmp(self, command):
        if not self.monitor:
            self.send("\x01c\n")
            self.expect("(qemu)")
            self.monitor = True
        self.send(command + "\n")
        return self.expect("(qemu)")

    def serial(self, enter=True):
        # The newline after leaving the monitor reaches the program in front as Enter; `enter=False` leaves it out.
        if self.monitor:
            self.send("\x01c\n" if enter else "\x01c")
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


def font16():
    import sys
    sys.path.insert(0, str(ROOT / "scripts"))
    import font_gen
    return font_gen.parse_bdf(font_gen.SUBSET.read_text(encoding="utf-8"))[1]


def glyph_lookup():
    """Bitmap -> character; glyphs drawn alike (Latin o and Cyrillic о) map to the lowest code point."""
    lookup = {}
    for code, (_, rows) in sorted(font16().items(), reverse=True):
        lookup[tuple(rows)] = chr(code)
    return lookup


def canon(text):
    """`text` as `screen_text` reads it back (look-alike letters folded)."""
    glyphs, lookup = font16(), glyph_lookup()
    return "".join(lookup[tuple(glyphs[ord(ch)][1])] if ord(ch) in glyphs else "?" for ch in text)


def screen_text(vm):
    """Reads the screen as text: every 8x16 cell with at most two colours is matched against the font's glyphs."""
    lookup = glyph_lookup()
    _, size, _, pixels = vm.screenshot().split(b"\n", 3)
    width, height = map(int, size.split())
    lines = []
    for cy in range(height // 16):
        line = []
        for cx in range(width // 8):
            rows = [[pixels[((cy * 16 + r) * width + cx * 8 + c) * 3:((cy * 16 + r) * width + cx * 8 + c) * 3 + 3] for c in range(8)] for r in range(16)]
            colours = {p for row in rows for p in row}
            found = " " if len(colours) == 1 else "?"
            for fg in colours if len(colours) == 2 else ():
                bits = tuple(sum(0x80 >> c for c in range(8) if row[c] == fg) for row in rows)
                ch = lookup.get(bits)
                if ch is not None:
                    found = ch
                    break
            line.append(found)
        lines.append("".join(line))
    return lines


def check_text16(vm, x, y, text, color, background):
    """The screen shows `text` in the 8x16 font at (x, y), pixel for pixel."""
    glyphs = font16()
    _, size, _, pixels = vm.screenshot().split(b"\n", 3)
    width = int(size.split()[0])
    fg, bg = color.to_bytes(3, "big"), background.to_bytes(3, "big")
    for index, ch in enumerate(text):
        rows = glyphs.get(ord(ch), glyphs[0xFFFD])[1]
        for row in range(16):
            for col in range(8):
                at = ((y + row) * width + x + index * 8 + col) * 3
                want = fg if rows[row] & (0x80 >> col) else bg
                assert pixels[at:at + 3] == want, (ch, index, row, col, pixels[at:at + 3], want)


def require(text, fragment):
    assert fragment in text, (fragment, text)


def hold_frames(vm, frames, leave_mib):
    """Starts memtest instances that hold all but about `leave_mib` of the frame pool; returns their PIDs."""
    holders, left = [], frames - leave_mib * 1024 * 1024
    while (mib := min(144, left // (1024 * 1024)) // 16 * 16) > 0:
        output = vm.command(f"run memtest hold {mib} &")
        holders.append(int(re.search(r"STARTED PID=(\d+)", output)[1]))
        for _ in range(80):
            if f"HELD {mib} MiB" in vm.command(f"logs {holders[-1]}"):
                break
            time.sleep(.25)
        else:
            raise AssertionError(f"memtest did not hold {mib} MiB")
        left -= mib * 1024 * 1024
    return holders


def frames_free(vm):
    # Free bytes of the frame pool for task memory (issue 150), once two readings agree.
    previous = None
    for _ in range(20):
        free = int(re.search(r"FRAMES=\d+ FRAMES_FREE=(\d+)", vm.command("free"))[1])
        if free == previous:
            return free
        previous = free
        time.sleep(.1)
    raise AssertionError("frame pool use did not settle")


def heap_used(vm):
    # IDL clients allocate a buffer per call (log lines of the services, for instance), so a reading can catch one in
    # flight: the value counts once two readings in a row agree.
    previous = None
    for _ in range(20):
        output = vm.command("heap")
        require(output, "TEST FREED=true")
        used = int(re.search(r"HEAP: USED=(\d+)", output).group(1))
        if used == previous:
            return used
        previous = used
        time.sleep(.1)
    return previous


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


def tablet_suite(args, disk):
    """Issue 160: a VirtIO tablet, an absolute pointer, needs no grab; its events carry the position."""
    path = Path(tempfile.gettempdir()) / f"mind-core-qmp-{os.getpid()}.sock"
    vm = VM(args, disk.relative_to(ROOT).as_posix(), extra=["-device", "virtio-tablet-pci", "-qmp", f"unix:{path},server=on,wait=off"])
    try:
        require(vm.service_logs("virtio_input", "TABLET READY"), "[VIRTIO_INPUT] TABLET READY")
        vm.send("run keys\n")
        vm.expect("[KEYS] READY")
        start = len(vm.log)
        # The monitor's mouse_move is relative only: absolute events go through QMP (0..0x7FFF on each axis).
        qmp = socket.socket(socket.AF_UNIX); qmp.connect(str(path)); reader = qmp.makefile("r")
        def execute(command, arguments=None):
            qmp.sendall(json.dumps({"execute": command, **({"arguments": arguments} if arguments else {})}).encode() + b"\n")
            while "return" not in (reply := json.loads(reader.readline())) and "error" not in reply:
                pass
            assert "error" not in reply, reply
        reader.readline()  # greeting
        execute("qmp_capabilities")
        events = [[{"type": "abs", "data": {"axis": "x", "value": 16383}}, {"type": "abs", "data": {"axis": "y", "value": 8191}}],
                  [{"type": "btn", "data": {"down": True, "button": "left"}}], [{"type": "btn", "data": {"down": False, "button": "left"}}],
                  [{"type": "btn", "data": {"down": True, "button": "wheel-down"}}], [{"type": "btn", "data": {"down": False, "button": "wheel-down"}}]]
        for batch in events:
            execute("input-send-event", {"events": batch})
            time.sleep(.15)
        qmp.close()
        time.sleep(.3)
        vm.collect()
        got = re.findall(r"\[KEYS\] (pointer [^\r\n]*)", vm.log[start:])
        positions = [tuple(map(int, m)) for m in re.findall(r"at=(\d+),(\d+)", " ".join(got))]
        assert positions and all(0 <= x < 4096 and 0 <= y < 4096 for x, y in positions), got
        assert any(abs(x - 2048) < 64 and abs(y - 1024) < 64 for x, y in positions), got
        assert any(line.startswith("pointer buttons=1 at=") for line in got), got
        assert any(line.endswith("wheel=1") or line.endswith("wheel=-1") for line in got), got
        vm.send_bytes(b"\x1b")
        vm.expect("EXITED. SHELL RESUMED.")
        print("PASS: VirtIO tablet: absolute positions in 1/4096 of the screen, buttons and wheel reach the focused program; no pointer grab needed", flush=True)
    finally:
        vm.close()
        log = Path(tempfile.gettempdir()) / "mind-core-tablet.log"
        log.write_text(vm.log)
        print(f"QEMU log: {log}", flush=True)


def keys_suite(vm):
    """Key events: the same events from the UART (VT100 sequences, UTF-8) and from PS/2, layouts, Esc."""
    baseline = heap_used(vm)
    vm.send("run keys\n")
    # A program without process control cannot take keys from others (issue 154).
    require(vm.expect("[KEYS] READY"), "[KEYS] LISTEN: Err(Rights)")
    uart = [(b"\x1b[A", "code=Up mods=-"), (b"\x1b[1;2C", "code=Right mods=S"), (b"\x1bOP", "code=F(1) mods=-"),
            (b"\x1b[15~", "code=F(5) mods=-"), (b"\x1b[24~", "code=F(12) mods=-"), (b"\x1b[3;5~", "code=Delete mods=C"),
            (b"\x1b[5~", "code=PageUp mods=-"), (b"\x1b[H", "code=Home mods=-"), (b"\x7f", "code=Backspace mods=- char=U+0008"),
            (b"\x03", "code=Char mods=C char=c"), ("Ж".encode(), "code=Char mods=- char=Ж U+0416"), (b"q", "code=Char mods=- char=q U+0071"),
            (b"\r\n", "code=Enter mods=- char=U+000A")]
    for data, line in uart:
        vm.send_bytes(data)
        vm.expect(line)
    # One CRLF is one Enter: the next event is the next key, not a second Enter.
    vm.send_bytes(b"x")
    assert "code=Enter" not in vm.expect("char=x U+0078")
    ps2 = [("up", "code=Up mods=-"), ("shift-right", "code=Right mods=S"), ("f1", "code=F(1) mods=-"), ("f10", "code=F(10) mods=-"),
           ("delete", "code=Delete mods=-"), ("home", "code=Home mods=-"), ("pgdn", "code=PageDown mods=-"), ("insert", "code=Insert mods=-"),
           ("ctrl-c", "code=Char mods=C char=c"), ("alt-x", "code=Char mods=A char=x"), ("shift-a", "code=Char mods=S char=A"),
           ("backspace", "code=Backspace mods=-"), ("tab", "code=Tab mods=-")]
    # Russian layout: Ctrl+Shift pressed and released alone switches it; letters by position; Alt+Shift switches back.
    ps2 += [("ctrl-shift", None), ("q", "char=й U+0439"), ("shift-q", "char=Й U+0419"), ("grave_accent", "char=ё U+0451"),
            ("ctrl-c", "code=Char mods=C char=c"), ("alt-shift", None), ("q", "char=q U+0071")]
    # The program's output arrives while the harness is in the QEMU monitor: check the whole log, in order.
    start = len(vm.log)
    for key, _ in ps2:
        vm.hmp(f"sendkey {key}")
        time.sleep(.05)
    vm.serial()
    time.sleep(.3)
    vm.collect()
    got = re.findall(r"\[KEYS\] (code=[^\r\n]*)", vm.log[start:])
    expected = [line for _, line in ps2 if line]
    at = 0
    for line in expected:
        while at < len(got) and line not in got[at]:
            at += 1
        assert at < len(got), (line, got)
        at += 1
    # The PS/2 mouse (issue 156): movement, a button, the wheel reach the focused program that asked for them.
    start = len(vm.log)
    for command in ("mouse_move 10 5", "mouse_button 1", "mouse_button 0", "mouse_move 0 0 1"):
        vm.hmp(command)
        time.sleep(.1)
    vm.serial()
    time.sleep(.3)
    vm.collect()
    got = re.findall(r"\[KEYS\] (pointer [^\r\n]*)", vm.log[start:])
    assert "pointer buttons=0 dx=10 dy=5 wheel=0" in got, got
    assert any(line.startswith("pointer buttons=1 ") for line in got), got
    assert any(line.endswith("wheel=1") or line.endswith("wheel=-1") for line in got), got
    vm.output = ""
    # The text UI (mind::tui) on the real screen: frame, title, the latest event, the key bar.
    screen = screen_text(vm)
    vm.serial()
    assert screen[0].startswith("╔") and canon(" keys — коды клавиш ") in screen[0], screen[0]
    assert any("code=Char mods=- char=q U+0071" in row for row in screen), screen
    assert screen[-1].startswith(canon(" Esc — выход")), screen[-1]
    # Esc from the UART after the sequence timeout ends the program.
    vm.send_bytes(b"\x1b")
    require(vm.expect("EXITED. SHELL RESUMED."), "[KEYS] DONE")
    time.sleep(.1); vm.collect(); vm.output = ""
    log = vm.service_logs("ps2_kbd", "[KBD] LAYOUT EN")
    require(log, "[KBD] LAYOUT RU"); require(log, "[KBD] MOUSE ON THE AUXILIARY PORT WITH A WHEEL")
    assert task_rows(vm) == {}
    assert heap_used(vm) == baseline
    # keymap (issue 085): the layout and its switch through the shell's keyboard client (idl/keyboard.wit).
    require(vm.command("keymap"), "LAYOUT: US  SWITCH: CTRL+SHIFT OR ALT+SHIFT  LAYOUTS: US RU")
    require(vm.command("keymap ru"), "LAYOUT: RU  SWITCH: CTRL+SHIFT OR ALT+SHIFT")
    require(vm.command("keymap --switch caps"), "LAYOUT: RU  SWITCH: CAPS LOCK")
    require(vm.command("keymap --switch sideways"), "USAGE: KEYMAP")
    # Pointer events while the shell has the focus (it did not ask for them) are dropped, not kept for the next program.
    vm.hmp("mouse_move 7 7"); vm.serial(); time.sleep(.2)
    vm.send("run keys\n")
    vm.expect("[KEYS] READY")
    time.sleep(.3); vm.collect()
    assert "pointer" not in vm.log[vm.log.rindex("[KEYS] READY"):], vm.log[-500:]
    start = len(vm.log)
    # Russian at once; Ctrl+Shift no longer switches; Caps Lock switches (and locks no capitals).
    for key in ("q", "ctrl-shift", "q", "caps_lock", "q"):
        vm.hmp(f"sendkey {key}")
        time.sleep(.05)
    vm.serial()
    time.sleep(.3)
    vm.collect()
    got = re.findall(r"\[KEYS\] (code=[^\r\n]*)", vm.log[start:])
    typed = [line for line in got if line.startswith("code=Char")]  # leaving the QEMU monitor also sends an Enter
    assert [re.search(r"char=(\S+)", line)[1] for line in typed] == ["й", "й", "q"] and all("mods=-" in line for line in typed), got
    vm.send_bytes(b"\x1b")
    require(vm.expect("EXITED. SHELL RESUMED."), "[KEYS] DONE")
    time.sleep(.1); vm.collect(); vm.output = ""
    require(vm.command("keymap us --switch both"), "LAYOUT: US  SWITCH: CTRL+SHIFT OR ALT+SHIFT")
    require(vm.service_logs("ps2_kbd", "[KBD] SWITCH CtrlOrAltShift"), "[KBD] SWITCH CapsLock")
    # A modifier held on the PS/2 keyboard reaches the program on its own: fm's key bar shows what Shift does.
    vm.send("fm\n")
    status_line(vm, "[FM] READY")
    plain = screen_text(vm)[-1]
    vm.hmp("sendkey shift 3000")  # held for 3 s
    time.sleep(.8)
    held = screen_text(vm)[-1]
    time.sleep(3)
    released = screen_text(vm)[-1]
    vm.serial()
    assert plain.startswith("1Help") and "4Edit" in plain, plain
    assert "4New" in held and "Help" not in held and "Edit" not in held, held
    assert released == plain, released
    vm.send_bytes(b"\x1b[21~")
    require(vm.expect("EXITED. SHELL RESUMED."), "[FM] DONE")
    time.sleep(.1); vm.collect(); vm.output = ""
    print("PASS: key events: VT100/xterm sequences and UTF-8 from the UART, E0 keys, F-keys and modifiers from PS/2, CRLF, Russian layout switch, Esc; "
          "keymap sets the layout and the switch key; a held Shift changes fm's key bar", flush=True)


def shell_suite(vm):
    """The shell's line editor, history, completion, Cyrillic and scrollback, from the UART and from PS/2."""
    def keys(data, fragment):
        # Redraws of the edited line also print the prompt: wait for the command's output followed by a prompt.
        vm.send_bytes(data)
        deadline = time.monotonic() + 8
        while time.monotonic() < deadline:
            vm.collect()
            seen = to_ordinal(ANSI.sub("", vm.output).replace("\r", ""))
            at = seen.find(fragment)
            if at >= 0 and seen.rfind("MIND> ") > at:
                vm.output = ""
                return seen
            time.sleep(.01)
        raise AssertionError(f"Timeout waiting for {fragment!r} and a prompt: {vm.output[-3000:]}")
    # Edit in the middle: type "ist", go Home, insert "l", go End, Enter -> "list".
    keys(b"ist\x1b[Hl\x1b[F\r", "PROGRAMS ON DISK (")
    # Delete: "cpusX", Left, Delete -> "cpus".
    keys(b"cpusX\x1b[D\x1b[3~\r", "CPU=0 APIC=")
    vm.command("clock")
    # History: Up twice is "cpus".
    keys(b"\x1b[A\x1b[A\r", "CPU=0 APIC=")
    # Esc clears a typed line.
    vm.send_bytes(b"garbage\x1b")
    time.sleep(.2)
    keys(b"heap\r", "HEAP: USED=")
    # Tab completion of a program name after RUN, and of a command.
    keys(b"run dzen-c\t&\r", "PID=1 NAME=dzen-clock BACKGROUND")
    keys(f"kil\t{BASE + 1}\r".encode(), "KILLED PID=1")  # raw bytes: the harness does not translate the PID
    # Several matches are listed under the line.
    vm.send_bytes(b"c\t")
    listing = vm.expect("cpus")
    for word in ("clear", "clock"):
        require(listing, word)
    vm.send_bytes(b"\x1b")
    time.sleep(.2)
    # Cyrillic typed at the terminal is shown in the shell and reaches the command parser.
    keys("привет мир\r".encode(), "ERROR: UNKNOWN COMMAND")
    screen = screen_text(vm)
    vm.serial()
    assert any(row.startswith(canon("MIND> привет мир")) for row in screen), screen
    # PS/2: Up recalls the last command, Enter runs it.
    vm.hmp("sendkey up")
    vm.hmp("sendkey ret")
    vm.serial()
    time.sleep(.3)
    vm.collect()
    assert vm.log.count("ERROR: UNKNOWN COMMAND") >= 2, "the recalled command ran again"
    vm.output = ""
    # Scrollback: after enough output the banner is off the screen; Shift+PgUp brings it back.
    for _ in range(6):
        vm.command("help")
    assert not any(canon("MIND CORE v1.6") in row for row in screen_text(vm))
    for _ in range(12):
        vm.hmp("sendkey shift-pgup")
    screen = screen_text(vm)
    assert any(canon("MIND CORE v1.6") in row for row in screen), screen
    vm.hmp("sendkey shift-pgdn")
    vm.serial()
    print("PASS: shell line editing (Home/End/Left/Delete), history, Esc, Tab completion, Cyrillic input and display, PS/2 history, scrollback", flush=True)


NOTES = "".join(f"Строка {i}: съешь же ещё этих мягких французских булок, да выпей чаю. Line {i}.\n" for i in range(1, 301))


def tools_suite(vm):
    """Text tools on the 8x16 text UI: the viewer."""
    baseline = heap_used(vm)
    vm.send("view docs/notes.txt\n")
    require(vm.expect("[VIEW] TOP 0x0"), f"[VIEW] OPEN docs/notes.txt {len(NOTES.encode())} BYTES")
    time.sleep(.2)
    screen = screen_text(vm)
    vm.serial()
    assert canon("notes.txt") in screen[0] and canon("Стр 1 ") in screen[0], screen[0]
    assert screen[1].startswith(canon("Строка 1: съешь же ещё этих мягких")), screen[1]
    assert canon("10Quit") in screen[-1], screen[-1]
    rows = len(screen) - 2
    # Page down moves by a page less one line; the status follows.
    vm.send_bytes(b"\x1b[6~")
    vm.expect("[VIEW] TOP")
    time.sleep(.2)
    screen = screen_text(vm)
    vm.serial()
    lines_per_row = 1 if len(NOTES.splitlines()[0]) <= len(screen[0]) else 2
    assert canon(f"Стр {(rows - 1) // lines_per_row + 1} ") in screen[0], screen[0]
    # F7 search (case-insensitive), the match is highlighted on the top line.
    vm.send_bytes(b"\x1b[18~")
    time.sleep(.3)
    vm.send_bytes("СТРОКА 200:".encode() + b"\r")
    vm.expect("[VIEW] TOP")
    time.sleep(.3)
    screen = screen_text(vm)
    vm.serial()
    assert screen[1].startswith(canon("Строка 200:")), screen[1]
    assert canon("Стр 200 ") in screen[0], screen[0]
    # End shows the last line at the bottom of the page.
    vm.send_bytes(b"\x1b[F")
    vm.expect("[VIEW] TOP")
    time.sleep(.2)
    screen = screen_text(vm)
    vm.serial()
    assert any(row.startswith(canon("Строка 300:")) or canon("Line 300.") in row for row in screen[-3:-1]), screen[-3:]
    vm.send_bytes(b"\x1b")
    require(vm.expect("EXITED. SHELL RESUMED."), "[VIEW] DONE")
    time.sleep(.1); vm.collect(); vm.output = ""
    # Hex mode on a binary.
    vm.send("view kernel.elf\n")
    vm.expect("[VIEW] TOP 0x0")
    vm.send_bytes(b"\x1bOS")
    vm.expect("[VIEW] TOP 0x0")
    time.sleep(.2)
    screen = screen_text(vm)
    vm.serial()
    assert screen[1].startswith("00000000: 7F 45 4C 46 02 01 01"), screen[1]
    assert "HEX" in screen[0], screen[0]
    vm.send_bytes(b"\x1b")
    vm.expect("EXITED. SHELL RESUMED.")
    time.sleep(.1); vm.collect(); vm.output = ""
    require(vm.command("view nothing.txt"), "PID=")
    assert heap_used(vm) == baseline
    print("PASS: view: UTF-8 text with Cyrillic, paging, line numbers, search, end of file, hex mode, missing file", flush=True)
    monitors_check(vm)


def table_row(screen, pattern):
    return next((row for row in screen if re.search(pattern, row)), None)


def status_line(vm, text, timeout=8, raw=False, whole=False):
    # The line from `text` on (expect() may return before the line ends), or the `whole` line; `raw` keeps real PIDs.
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        vm.collect()
        clean = ANSI.sub("", vm.output).replace("\r", "")
        clean = clean if raw else to_ordinal(clean)
        at = clean.find(text)
        if at >= 0 and "\n" in clean[at:]:
            vm.output = ""
            return clean[clean.rfind("\n", 0, at) + 1 if whole else at:clean.index("\n", at)]
        time.sleep(.01)
    raise AssertionError(f"Timeout waiting for the line {text!r}: {vm.output[-3000:]}")


def tool_status(vm, text):
    # The state line a monitor logs after a key: x is bound in none of them. (The line after the Enter that leaving
    # the QEMU monitor sends may be dropped by serial().)
    vm.send("x")
    return status_line(vm, text)


def monitors_check(vm):
    """top, memmap, load and hw on sysmon's data. Each logs its state after every key; leaving the QEMU monitor after
    a screenshot sends Enter to the program in front."""
    baseline = heap_used(vm)
    clock = int(re.search(r"PID=(\d+) NAME=clock BACKGROUND", vm.command("run clock &"))[1])
    tasks = BASE + 2  # the services, clock and the monitor
    # top: the task table agrees with ps; details, sorting, filter and tree.
    vm.send("top\n")
    vm.expect("[TOP] READY")
    time.sleep(1.5)
    screen = screen_text(vm)
    vm.serial()  # Enter: the details window of the selected task
    assert canon(f"Tasks {tasks}:") in screen[1], screen[1]
    assert canon("load average") in screen[0], screen[0]
    assert table_row(screen, r"PID +PPID NAME +STATE") and table_row(screen, r" clock +") and table_row(screen, r" top +"), screen
    assert table_row(screen, r"/64\.0M used"), screen
    assert len([row for row in screen if re.match(r"^CPU\d|^ CPU\d", row)]) >= 1, screen
    assert re.search(r"DETAILS=[1-9]", tool_status(vm, "[TOP] SORT=CPU"))
    time.sleep(.3)
    screen = screen_text(vm)
    vm.serial()  # Enter closes it
    assert table_row(screen, canon("Capabilities")) and table_row(screen, canon("Address space:")), screen
    tool_status(vm, "DETAILS=0")
    vm.send("N")
    vm.expect("[TOP] SORT=PID")
    time.sleep(.2)
    screen = screen_text(vm)
    vm.serial()
    header = next(i for i, row in enumerate(screen) if "PPID" in row)
    assert re.match(r"^ +1 +0 init ", screen[header + 1]), screen[header + 1]
    assert re.search(r"DETAILS=[1-9]", tool_status(vm, "[TOP] SORT=PID"))
    vm.send_bytes(b"\x1b")
    vm.expect("DETAILS=0")
    vm.send("S")
    assert "ROWS=2" in status_line(vm, "HIDE=1"), "only clock and top are applications"
    vm.send("t")
    vm.expect("TREE=1")
    vm.send("q")
    require(vm.expect("EXITED. SHELL RESUMED."), "[TOP] DONE")
    time.sleep(.1); vm.collect(); vm.output = ""
    # memmap: physical map, kernel arena, the known layout of clock's address space, quotas.
    vm.send("memmap\n")
    vm.expect("[MEMMAP] READY")
    time.sleep(.3)
    screen = screen_text(vm)
    vm.serial()
    tool_status(vm, "[MEMMAP] VIEW=PHYSICAL")
    assert table_row(screen, canon("Physical address space")) and table_row(screen, canon("free RAM")), screen
    assert table_row(screen, r"0x[0-9a-f]{12} 0x[0-9a-f]{12} +64\.0M  kernel arena"), screen
    usable = re.search(r"RAM (\d+(?:\.\d)?)M usable", "\n".join(screen))
    assert usable and 128 < float(usable[1]) < 512, screen  # the VM has 512 MiB
    vm.send("2")
    vm.expect("VIEW=ARENA")
    time.sleep(.2)
    screen = screen_text(vm)
    vm.serial()
    assert table_row(screen, re.escape(canon("Kernel arena 64.0M: used"))) and table_row(screen, canon(f"Tasks {tasks}/32")), screen
    largest = table_row(screen, r"Kernel arena 64\.0M: used .*, largest free block (\d+(?:\.\d)?)M")
    assert largest and table_row(screen, r"Free outside the largest block: "), screen  # issue 076
    vm.send("3")
    vm.expect("VIEW=PROCESS")
    for _ in range(30):
        vm.send_bytes(b"\x1b[B")
        if f"PID={clock + BASE} " in status_line(vm, "VIEW=PROCESS", raw=True):
            break
    else:
        raise AssertionError("clock not in memmap's task list")
    time.sleep(.2)
    screen = screen_text(vm)
    vm.serial()
    assert table_row(screen, re.escape(canon(f"Address space of clock (PID {clock + BASE})"))), screen
    for line in (r"0x0000008000000000 +\S+ +r-x +image", r"0x0000008001000000 +4\.0K +--- +guard", r"0x0000008001001000 +64\.0K +rw- +stack",
                 r"0x0000008002000000 +\S+ +rw- +screen", r"0x0000008004000000 +4\.0K +r-- +info", r"0x0000008004001000 +4\.0K +rw- +mailbox", r"0x0000008005000000 +4\.0K +r-x +exit"):
        assert table_row(screen, line), (line, screen)
    vm.send("4")
    vm.expect("VIEW=QUOTAS")
    time.sleep(.2)
    screen = screen_text(vm)
    vm.serial()
    assert table_row(screen, r" loader +2/8 "), screen
    vm.send("q")
    require(vm.expect("EXITED. SHELL RESUMED."), "[MEMMAP] DONE")
    time.sleep(.1); vm.collect(); vm.output = ""
    # load: a graph per CPU and per counter; total CPU; the 10-minute window.
    vm.send("load\n")
    vm.expect("[LOAD] READY")
    time.sleep(1.5)
    screen = screen_text(vm)
    vm.serial()
    assert int(re.search(r"SAMPLES=(\d+)", tool_status(vm, "[LOAD] WINDOW=30S TOTAL=0"))[1]) > 10
    for name in [f"CPU{cpu} " for cpu in range(vm.cpus)] + ["interrupts ", "syscalls ", "IPC messages ", "context switches ", "kernel arena ", f"tasks  {tasks} of 32"]:
        assert table_row(screen, "^ " + re.escape(canon(name))), (name, screen)
    vm.send("c")
    vm.expect("TOTAL=1")
    time.sleep(.2)
    screen = screen_text(vm)
    vm.serial()
    assert table_row(screen, re.escape(canon(f"CPU total ({vm.cpus})"))), screen
    vm.send("2")
    vm.expect("WINDOW=10MIN")
    time.sleep(.2)
    screen = screen_text(vm)
    vm.serial()
    assert canon("10 min, 1 s samples") in screen[0], screen[0]
    vm.send("q")
    require(vm.expect("EXITED. SHELL RESUMED."), "[LOAD] DONE")
    time.sleep(.1); vm.collect(); vm.output = ""
    # hw: CPUID, clocks, the framebuffer, PCI devices and interrupt lines with their holders.
    vm.send("hw\n")
    vm.expect("[HW] READY")
    time.sleep(.3)
    screen = screen_text(vm)
    vm.serial()
    tool_status(vm, "[HW] TOP=0")
    for text in ("Processor", f"{vm.cpus} CPUs online", "+NX", "MHz (calibrated)", f"GOP framebuffer {len(screen[0]) * 8}x{len(screen) * 16}",
                 "00:01.1  010180  IDE controller", "Interrupt lines", "kernel arena 64.0M"):
        assert table_row(screen, re.escape(canon(text))), (text, screen)
    # The holder of a line is the driver; init keeps a copy for restarts (issues 075, 076).
    assert table_row(screen, r"IRQ 1 .* ps2_kbd \(PID \d+\) \(\+1 holding a copy\)"), screen
    vm.send("q")
    require(vm.expect("EXITED. SHELL RESUMED."), "[HW] DONE")
    time.sleep(.1); vm.collect(); vm.output = ""
    # ipc: endpoints with their servers, the holders of one, who waits for whom (issue 080).
    vm.send("ipc\n")
    vm.expect("[IPC] READY")
    time.sleep(.3)
    screen = screen_text(vm)
    vm.serial()  # Enter: the holders of the selected (first) endpoint
    assert table_row(screen, r"\d+ endpoints, .* messages, a queue holds at most 4 senders"), screen
    for service in ("vfs_server", "loader", "sysmon", "logd", "rtc"):
        assert table_row(screen, fr"^ +\d+ +{service} \(PID \d+\) +\d+ +\d/4 "), (service, screen)
    status = tool_status(vm, "[IPC] VIEW=ENDPOINTS SORT=INDEX")
    first, holders = int(re.search(r"SELECTED=(\d+)", status)[1]), int(re.search(r"HOLDERS=(\d+)", status)[1])
    time.sleep(.3)
    screen = screen_text(vm)
    vm.serial()  # Enter closes the window
    assert holders >= 1 and table_row(screen, re.escape(canon(f"Endpoint {first}: {holders} holders"))), (status, screen)
    tool_status(vm, "HOLDERS=0")
    vm.send("2")
    status_line(vm, "[IPC] VIEW=WAITS")
    time.sleep(.2)
    screen = screen_text(vm)
    vm.serial()  # Enter does nothing in this view
    assert table_row(screen, r"tasks wait for a message on their own endpoints; \d+ edges, 0 deadlocks"), screen
    vm.send("q")
    require(vm.expect("EXITED. SHELL RESUMED."), "[IPC] DONE")
    time.sleep(.1); vm.collect(); vm.output = ""
    # caps: a task's capabilities, the derivation tree and what a revoke removes, through the shell's authority client
    # (issue 081). `caps` without a PID starts the tool on the first task, init.
    vm.send("caps\n")
    vm.expect("[CAPS] READY")
    time.sleep(.3)
    screen = screen_text(vm)
    vm.serial()  # Enter: what a revoke of init's first capability removes
    assert table_row(screen, r"init \(PID 1\): \d+ capabilities"), screen
    status = tool_status(vm, "[CAPS] VIEW=TASK PID=1 ")
    entries, revoke = int(re.search(r"ENTRIES=(\d+)", status)[1]), int(re.search(r"REVOKE=(\d+)", status)[1])
    assert "DENIED=0" in status and entries > 50, status
    time.sleep(.3)
    screen = screen_text(vm)
    vm.serial()  # Enter closes the window
    assert table_row(screen, r"Revoke slot \d+ of init"), screen
    tool_status(vm, "REVOKE=-")
    # The tree: init's originals with the services' copies below them.
    vm.send("2")
    status_line(vm, "[CAPS] VIEW=TREE")
    time.sleep(.3)
    screen = screen_text(vm)
    vm.serial()
    tree = [row for row in screen if re.search(r"\(PID \d+\) +slot +\d+", row)]
    copies = [i for i, row in enumerate(tree) if re.match(r" +init \(PID 1\)", tree[i - 1] if i else "") and re.match(r"   +\w+ \(PID \d+\)", row)]
    assert copies, screen
    tool_status(vm, "[CAPS] VIEW=TREE")  # the Enter of serial() opened the revoke window of the first root
    vm.send("q")  # closes the window
    tool_status(vm, "REVOKE=-")
    vm.send("q")
    require(vm.expect("EXITED. SHELL RESUMED."), "[CAPS] DONE")
    time.sleep(.1); vm.collect(); vm.output = ""
    # The same program without REQUEST_AUTHORITY gets the plain sysmon client: sysmon refuses the authority graph.
    vm.send("capsobs\n")
    vm.expect("[CAPS] READY")
    time.sleep(.3)
    screen = screen_text(vm)
    vm.serial()
    assert table_row(screen, r"No authority"), screen
    status = tool_status(vm, "[CAPS] VIEW=")
    assert "ENTRIES=0" in status and "DENIED=1" in status, status
    vm.send("q")
    require(vm.expect("EXITED. SHELL RESUMED."), "[CAPS] DONE")
    time.sleep(.1); vm.collect(); vm.output = ""
    require(vm.command(f"kill {clock}"), "KILLED")
    for _ in range(20):
        if heap_used(vm) == baseline:
            break
        time.sleep(.1)
    assert heap_used(vm) == baseline
    print("PASS: monitors: top (task table = ps, details, sorting, filter, tree), memmap (physical map, arena, a known address space, quotas), load (graphs, total, 10 min), hw (CPUID, framebuffer, PCI, IRQ holders), ipc (endpoints, holders, waits)", flush=True)
    fm_check(vm)


def fm_check(vm):
    """The file manager: browse into EFI/BOOT and back, view a file, start a program from the panel."""
    baseline = heap_used(vm)
    def keys(data, text):
        vm.send_bytes(data)
        return status_line(vm, text)
    def poke(text):
        return keys(b"\x18", text)  # Ctrl+X: bound to nothing (a letter would go to the command line)
    vm.send("fm\n")
    vm.expect("[FM] READY LEFT=/ FULL RIGHT=/ BRIEF ACTIVE=L CURRENT=docs")
    time.sleep(.3)
    screen = screen_text(vm)
    vm.serial()  # Enter: into docs
    assert canon("A:/") in screen[0] and canon("10Quit") in screen[-1], (screen[0], screen[-1])
    assert table_row(screen, r"║EFI +│.SUB-DIR.│\d{4}-\d\d-\d\d│\d\d:\d\d║"), screen
    assert table_row(screen, r"║kernel\.elf +│ +\d+│\d{4}-\d\d-\d\d│"), screen
    assert "CURRENT=.." in poke("[FM] LEFT=/docs FULL")
    # F3 views notes.txt in the built-in viewer; Esc comes back.
    keys(b"\x1b[B", "CURRENT=notes.txt")
    keys(b"\x1bOR", "VIEW=1")
    time.sleep(.2)
    screen = screen_text(vm)
    vm.serial()
    assert screen[1].startswith(canon("Строка 1: съешь")), screen[1]
    keys(b"\x1b", "VIEW=0")
    # ".." goes up with the cursor on the directory left; EFI/BOOT and back.
    keys(b"\x1b[H\r", "LEFT=/ FULL")
    assert "CURRENT=docs" in poke("[FM] LEFT=/ FULL")
    keys(b"\x1b[B", "CURRENT=EFI")
    keys(b"\r", "LEFT=/EFI FULL")
    keys(b"\x1b[B", "CURRENT=BOOT")
    keys(b"\r", "LEFT=/EFI/BOOT FULL")
    poke("LEFT=/EFI/BOOT FULL")  # not CR last: CR LF would be one Enter
    time.sleep(.2)
    screen = screen_text(vm)
    vm.serial()  # Enter on "..": back to EFI
    assert canon("A:/EFI/BOOT") in screen[0] and table_row(screen, r"║BOOTX64\.EFI +│ +\d+│"), screen
    assert "CURRENT=BOOT" in poke("[FM] LEFT=/EFI FULL")
    keys(b"\x7f", "LEFT=/ FULL")
    # A program started from the panel runs in the background.
    for _ in range(60):
        if "CURRENT=clock.elf " in keys(b"\x1b[B", "[FM] LEFT=/ FULL"):
            break
    else:
        raise AssertionError("clock.elf not reached")
    keys(b"\r", "CURRENT=clock.elf")
    time.sleep(.2)
    screen = screen_text(vm)
    vm.serial()
    started = table_row(screen, canon("Started clock.elf as PID"))
    assert started, screen
    pid = int(re.search(r"PID (\d+)", started)[1])
    # The command line (issue 097): typing goes under the panels, Enter runs it (cd, edit, view, a program with its
    # arguments); Ctrl+O hides both panels and shows what it did there, Ctrl+F1 / Ctrl+F2 one, Ctrl+P the other.
    keys(b"cd docs", "CMD=cd docs")
    keys(b"\r", "LEFT=/docs FULL")
    keys(b"\x0f", "HIDDEN=LR")
    time.sleep(.2)
    screen = screen_text(vm)
    vm.serial()  # Enter on an empty line with the panels hidden: nothing
    assert any(line.startswith(canon("A:/> cd docs")) for line in screen) and not any(canon("║") in line for line in screen[:-2]), screen
    assert screen[-2].startswith(canon("A:/docs>")), screen[-2]
    keys(b"cd ..", "CMD=cd ..")
    keys(b"\r", "LEFT=/ FULL RIGHT=/ BRIEF ACTIVE=L CURRENT=docs")
    assert "HIDDEN" not in keys(b"\x0f", "CURRENT=docs"), "Ctrl+O shows the panels again"
    vm.send_bytes(b"\x1b[1;5P")
    assert "ACTIVE=R" in status_line(vm, "HIDDEN=L", whole=True), "Ctrl+F1 hides the left panel; the right one is active"
    time.sleep(.2)
    screen = screen_text(vm)
    vm.serial()
    assert any(line.startswith(canon("A:/docs> cd ..")) for line in screen) and canon("║") in screen[1][40:], screen
    assert screen[-1].startswith(canon("1Help")), "a terminal's Ctrl does not stay in the key bar: " + screen[-1]
    assert "HIDDEN" not in keys(b"\x10", "ACTIVE=R"), "Ctrl+P shows the other panel again"
    keys(b"\t", "ACTIVE=L")
    keys(b"view docs/notes.txt", "CMD=view docs/notes.txt")
    keys(b"\r", "VIEW=1")
    keys(b"\x1b", "VIEW=0")
    vm.send_bytes(b"\x1b[21~")
    require(vm.expect("EXITED. SHELL RESUMED."), "[FM] DONE")
    time.sleep(.1); vm.collect(); vm.output = ""
    assert pid - BASE in task_rows(vm), task_rows(vm)
    time.sleep(1.2)
    require(vm.command(f"logs {pid - BASE}"), "[CLOCK] ")
    require(vm.command(f"kill {pid - BASE}"), "KILLED")
    for _ in range(20):
        if heap_used(vm) == baseline:
            break
        time.sleep(.1)
    assert heap_used(vm) == baseline
    print("PASS: fm: two panels with sizes and dates, the built-in viewer, EFI/BOOT and back, a program started from the panel, the command line, Ctrl+O, Ctrl+F1 and Ctrl+P", flush=True)
    vfs_check(vm)


def vfs_check(vm):
    """Files through the shell (VFS v2, idl/vfs.wit): the RAM disk, Cyrillic names and text, directories, and what the
    user's badge may not change."""
    def utf8(line):
        vm.send_bytes((line + "\n").encode())
        return vm.expect("MIND> ")
    require(vm.command("ls ram:"), "0 ENTRIES")
    require(utf8("write ram:заметки.txt Привет, мир"), "WROTE 21 BYTES")
    require(utf8("cat ram:заметки.txt"), "Привет, мир")
    require(vm.command("mkdir ram:docs/old"), "OK")
    require(utf8("mv ram:заметки.txt ram:docs/old/note.txt"), "OK")
    listing = vm.command("ls ram:docs/old")
    require(listing, "note.txt")
    require(listing, "1 FILES, 21 BYTES")
    require(vm.command("rm ram:docs"), "ERROR: RM: DIRECTORY NOT EMPTY")
    require(vm.command("rm ram:docs/old/note.txt"), "OK")
    require(vm.command("rm ram:docs/old"), "OK")
    # Boot files and the rest of the boot disk stay read-only; paths cannot climb out; data/ is writable.
    require(vm.command("write kernel.elf x"), "ERROR: WRITE: DENIED")
    require(vm.command("write EFI/BOOT/x.txt x"), "ERROR: WRITE: DENIED")
    require(vm.command("mkdir system"), "ERROR: MKDIR: DENIED")
    require(vm.command("cat ram:../kernel.elf"), "ERROR: CAT: INVALID PATH")
    require(vm.command("rm kernel.elf"), "ERROR: RM: DENIED")
    require(vm.command("mkdir data"), "OK")
    require(vm.command("write data/n.txt hello"), "WROTE 6 BYTES")
    require(vm.command("cat data/n.txt"), "hello")
    require(vm.command("ls data"), "n.txt")
    print("PASS: files: RAM disk with Cyrillic names and text, mkdir with parents, move, remove; boot files and the disk outside data/ are not writable, .. is refused", flush=True)
    search_check(vm)


def search_check(vm):
    """find and grep (issue 082): console programs over the application's read-only file client."""
    def utf8(line):
        vm.send_bytes((line + "\n").encode())
        return vm.expect("MIND> ")
    def lines(output):
        skip = ("MIND>", "find ", "grep ", "STARTED PID=")
        return [l.strip() for l in output.replace("\r", "").split("\n") if l.strip() and not l.startswith(skip) and "EXITED. SHELL RESUMED" not in l]
    require(vm.command("write ram:a.txt hello world"), "WROTE")
    require(vm.command("mkdir ram:docs"), "OK")
    require(utf8("write ram:docs/заметки.txt Привет, мир"), "WROTE")
    require(vm.command("write ram:docs/b.md # title"), "WROTE")
    assert lines(vm.command("find ram:")) == ["ram:a.txt", "ram:docs/", "ram:docs/b.md", "ram:docs/заметки.txt"], vm.output
    assert lines(vm.command("find ram: -name *.txt -type f")) == ["ram:a.txt", "ram:docs/заметки.txt"]
    assert lines(vm.command("find ram: -size +12")) == ["ram:docs/заметки.txt"], "20 bytes of Cyrillic text"
    assert lines(vm.command("find A: -name kernel.elf")) == ["kernel.elf"]
    assert lines(utf8("grep -rn привет ram:")) == [], "case matters without -i"
    assert lines(utf8("grep -rin привет ram:")) == ["ram:docs/заметки.txt:1:Привет, мир"]
    assert lines(utf8("grep -l o ram:a.txt ram:docs/заметки.txt ram:docs/b.md")) == ["ram:a.txt"]
    assert lines(vm.command("grep -c ^# ram:docs/b.md")) == ["1"]
    assert lines(vm.command("grep ELF kernel.elf")) == ["BINARY FILE kernel.elf MATCHES"]
    require(vm.command("grep x ram:docs"), "GREP: ram:docs: IS A DIRECTORY (USE -R)")
    require(vm.command("find -bogus"), "FIND: UNKNOWN OPTION -bogus")
    for path in ("ram:docs/b.md", "ram:a.txt"):
        require(vm.command(f"rm {path}"), "OK")
    require(utf8("rm ram:docs/заметки.txt"), "OK")
    require(vm.command("rm ram:docs"), "OK")
    print("PASS: find and grep: names, types and sizes on ram: and the boot disk; Cyrillic text with -i; -n, -l, -c, -r; a binary file", flush=True)


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
    # sysmon's samples see the busy CPU: one of the CPUs at full load.
    time.sleep(1.5)
    cpu = int(re.search(r"cpu (\d+)%", vm.command("uptime"))[1])
    assert cpu >= 100 // vm.cpus // 2, cpu
    # top sees the busy loop at about 100 % of its CPU.
    vm.send("top\n")
    vm.expect("[TOP] READY")
    time.sleep(2.2)
    screen = screen_text(vm)
    vm.serial()
    row = next((r for r in screen if re.search(r" app2 ", r)), None)
    assert row and float(row.split()[5]) >= 80, (row, screen)
    if "DETAILS=0" not in tool_status(vm, "[TOP] SORT"):  # opened by the Enter after the screenshot
        vm.send_bytes(b"\x1b")
        vm.expect("DETAILS=0")
    vm.send("q")
    vm.expect("EXITED. SHELL RESUMED.")
    time.sleep(.1); vm.collect(); vm.output = ""
    # TSC accounting (STAT): a task that never yields gets most of its CPU.
    run = lambda: int(re.search(r"RUN_MS=(\d+)", vm.command("stat 1"))[1])
    before, started = run(), time.monotonic()
    time.sleep(1)
    after, elapsed = run(), time.monotonic() - started
    assert (after - before) > 0.3 * elapsed * 1000, (before, after, elapsed)
    fixture = vm.command("logs 1")
    require(fixture, "BUSY FIXTURE")
    avx_expected(vm, fixture)
    # Scheduling budget (C7): 20 ms per 100 ms keeps the busy loop near 20 % of its CPU (enforced at the 10 ms tick).
    require(vm.command("budget 1 20 100"), "BUDGET PID=1 20 MS PER 100 MS")
    def run_ms():
        return int(re.search(r"^\d+ PARENT=\d+ app2 WAIT=\S+ CPU=\d+ RUN_MS=(\d+)", vm.command("stat tasks", raw=True), re.M)[1])
    start_run, start = run_ms(), time.monotonic()
    time.sleep(3)
    share = (run_ms() - start_run) / ((time.monotonic() - start) * 1000)
    assert 0.12 < share < 0.35, share
    require(vm.command("budget 1 0 0"), "BUDGET PID=1 0 MS PER 0 MS")
    start_run, start = run_ms(), time.monotonic()
    time.sleep(1)
    assert (run_ms() - start_run) / ((time.monotonic() - start) * 1000) > 0.6, "no budget: the loop takes most of its CPU"
    require(vm.command("kill 1"), "KILLED PID=1")
    vm.command("kill 2")
    assert heap_used(vm) == baseline
    print("PASS: timer preemption of a non-yielding SIMD loop; responsive shell, clocks and kill; top shows the loop at ~100 % of its CPU; CPU budget per period", flush=True)


def avx_expected(vm, fixture=None):
    """With a CPU model that has AVX (--cpu-model max) every CPU saves AVX state and the busy fixture uses AVX."""
    cpus = vm.command("cpus")
    model = getattr(vm.args, "cpu_model", None)
    if model == "max":
        assert len(re.findall(r"FPU=XSAVE\+AVX", cpus)) == vm.cpus, cpus
        assert fixture is None or "CALLS, AVX" in fixture, fixture
    elif model is None:
        assert len(re.findall(r"FPU=FXSAVE", cpus)) == vm.cpus, cpus


def smp_suite(vm):
    baseline = heap_used(vm)
    cpus = vm.command("cpus")
    assert len(re.findall(r"ONLINE=true", cpus)) == vm.cpus, cpus
    avx_expected(vm)
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
    # Reserve (C7): with every CPU saturated by applications, init (system band) still restarts a killed service.
    rtc = vm.services()["rtc"]
    start = time.monotonic()
    require(vm.command(f"kill {rtc}", raw=True), "KILLED PID=")
    require(vm.service_logs("init", "rtc RESTARTED"), "rtc RESTARTED")
    assert time.monotonic() - start < 5, "init must not wait for the applications"
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
    print(f"PASS: {vm.cpus} online CPUs, concurrent pinned tasks, SIMD preservation, supervisor reserve under load, remote kill, all CPUs HLT", flush=True)


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
    for key, children, done in [("q", 5, "QUEUE BOUND OK"), ("j", 1, "LATE REPLY OK"), ("z", 1, "MOVE OK"), ("b", 1, "REVOKE PENDING OK"), ("i", 1, "BADGE OK")]:
        pid, faults = family(key, children, done)
        assert not any(f"FAULT PID={pid + n} " in faults for n in range(children + 1)), (key, faults)
    # The child keeps reading a lease when the parent revokes it: its next access faults (CAP_REVOKE waits for its CPU).
    pid, faults = family("x", 1, "LEASE REVOKED")
    assert re.search(fr"FAULT PID={pid + 1} CPU=\d+ VECTOR=14 ", faults), faults
    assert int(task_rows(vm)[1][-1]) > int(before[-1])
    vm.command("kill 1")
    assert heap_used(vm) == baseline, "fault teardown leaked task/page-table resources"
    require(vm.command("run app &"), "NAME=app BACKGROUND")
    print("PASS: CPL3/IOPL0; kernel read/write, RX code, NX stack, CLI/I/O, UD2, guard/bad stack; syscall pointers; capability checks and endpoint badges; fault containment and reclaim", flush=True)


def memory_suite(vm):
    baseline, frames = heap_used(vm), frames_free(vm)
    # Task memory comes from the frame pool (issue 150): memtest holds all but about 40 MiB of it, so spawns run out.
    holders = hold_frames(vm, frames, 40)
    pids = []
    for _ in range(8 - len(holders)):
        before, frames_before = heap_used(vm), frames_free(vm)
        output = vm.command("run app2 &")
        if "OUT OF MEMORY" in output:
            assert heap_used(vm) == before and frames_free(vm) == frames_before, "partial spawn must roll back all allocations"
            break
        match = re.search(r"STARTED PID=(\d+)", output)
        assert match, output
        pids.append(int(match[1]))
    else:
        raise AssertionError("large-BSS fixture did not exercise allocation failure")
    assert 1 < len(pids) < 8 - len(holders), pids
    first = task_rows(vm)
    time.sleep(.3)
    second = task_rows(vm)
    assert all(int(second[p][-1]) > int(first[p][-1]) for p in pids)
    for pid in pids + holders:
        vm.command(f"kill {pid}")
    assert heap_used(vm) == baseline and frames_free(vm) == frames
    require(vm.command("run clock &"), "NAME=clock BACKGROUND")
    memory_beyond_the_arena(vm)
    print("PASS: out-of-memory rollback, surviving tasks and later successful launch", flush=True)


def memory_beyond_the_arena(vm):
    """Issue 150: task memory comes from the frame pool, not the 64 MiB arena; a program asks for its quota."""
    used, frames = heap_used(vm), frames_free(vm)
    vm.send("memtest alloc 128\n")
    output = vm.expect("MIND> ", timeout=120, after="memtest alloc 128\n")
    require(output, "[MEMTEST] HELD 128 MiB INTACT=true")
    require(output, "[MEMTEST] BEYOND QUOTA: REFUSED")
    require(output, "[MEMTEST] FREED")
    assert frames_free(vm) == frames and heap_used(vm) == used
    # A 64 MiB sealed object mapped read-only by two tasks; the frames come back when both let go.
    vm.send("memtest share 64\n")
    output = vm.expect("MIND> ", timeout=120, after="memtest share 64\n")
    require(output, "[MEMTEST] SEALED=true")
    require(output, "[MEMTEST] PARENT MAPPED 64 MiB INTACT=true")
    require(output, "[MEMTEST] CHILD MAPPED 64 MiB READ-ONLY SEALED=true INTACT=true SAME SUM=true")
    require(output, "[MEMTEST] SHARE DONE")
    for _ in range(40):
        if frames_free(vm) == frames:
            break
        time.sleep(.25)
    else:
        raise AssertionError("the shared object's frames were not returned")
    print("PASS: 128 MiB of heap beyond the arena within the requested quota (one block more refused); a 64 MiB sealed object mapped by two tasks; frames returned", flush=True)


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
    # The frame pool (issue 150) is larger than four quotas: memtest holds all but about 40 MiB of it.
    hogs = hold_frames(vm, frames_free(vm), 40)
    next_pid += len(hogs)
    holders, failed = [], []
    for pid in clients:
        before, frames_before = heap_used(vm), frames_free(vm)
        foreground(pid)
        vm.send("b\n")
        output = vm.expect("HEAP ALLOCATION FINISHED")
        vm.background(pid)
        if "OOM" in output:
            failed.append(pid)
            assert heap_used(vm) == before and frames_free(vm) == frames_before, "failed allocation changed heap usage"
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
    for pid in clients + hogs:
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
    # The VM's RTC was set to 2026-09-19T19:35:05.
    require(vm.command("date"), "DATE: 2026-09-19 19:35:")
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
    require(output, "(SORTED, HEAP ARENAS=1)")  # Vec/String on the program heap (mind::alloc)
    names = re.findall(r"^\[FILES\] (\S+) \d+$", output, re.M)
    assert names == sorted(names), names
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
    # The shell's fixed grants 13..15 (issue 151): sysmon's authority view (badged), the keyboard driver, the compositor.
    real = vm.services()
    caps = vm.command(f"stat caps {real['shell']}", raw=True)
    servers = {int(ep): int(server) for ep, server in re.findall(r"^EP=(\d+) .*SERVER=(\d+)", vm.command("endpoints", raw=True), re.M)}
    for slot, service, badge in [(13, "sysmon", 1), (14, "ps2_kbd", 0), (15, "compositor", 0)]:
        found = re.search(fr"^SLOT={slot} GEN=0 KIND=1 RIGHTS=6 SIZE=0 BADGE={badge} EP=(\d+)", caps, re.M)
        assert found and servers.get(int(found[1])) == real[service], (slot, service, caps)
    output = vm.command("ps")
    for name in ("rtc", "ps2_kbd", "compositor", "ata", "vfs_server", "loader", "audio_gw", "tts", "sysmon"):
        assert re.search(fr"^\d+ {name} (IPC_WAIT|IRQ_WAIT|SLEEPING|READY|RUNNING) BG", output, re.M), (name, output)
    # Monotonic clock: calibrated TSC with sub-millisecond resolution, never going backwards.
    clocks = [re.search(r"MONOTONIC NS=(\d+) RESOLUTION NS=(\d+) TSC HZ=(\d+)", vm.command("clock")) for _ in range(2)]
    assert all(clocks), clocks
    (first, resolution, hz), (second, _, _) = [tuple(map(int, c.groups())) for c in clocks]
    assert second > first and 0 < resolution < 1_000_000 and hz > 1_000_000, (first, second, resolution, hz)
    # Observation (STAT): the task table agrees with ps, the memory summary with heap, and every CPU is online.
    tasks = len(re.findall(r"^\d+ [\w-]+ ", vm.command("ps", raw=True), re.M))
    free = vm.command("free")
    assert f"TASKS={tasks}/32 " in free and re.search(r"ENDPOINTS=\d+/127 ", free), (tasks, free)
    arena, used, free_bytes, largest = map(int, re.search(r"ARENA=(\d+) USED=(\d+) FREE=(\d+) LARGEST=(\d+)", free).groups())
    assert arena == 64 << 20 and 0 < used < arena and used + free_bytes <= arena, free
    # Issue 075: the largest free block (found by trial allocations) fits in the free memory; page tables are counted.
    assert 4096 <= largest <= free_bytes and int(re.search(r"PAGE_TABLES=(\d+)", free)[1]) > 0, free
    cpus = vm.command("cpus")
    assert len(re.findall(r"BUSY_MS=\d+ IDLE_MS=\d+ SWITCHES=\d+", cpus)) == vm.cpus, cpus
    physmap = vm.command("physmap")
    for kind in ("free RAM", "kernel arena", "kernel image", "framebuffer", "boot image", "AP trampoline"):
        require(physmap, kind)
    free_ram = int(re.search(r"FREE_RAM=(\d+)K", physmap)[1])
    assert 128 * 1024 < free_ram < 512 * 1024, free_ram  # the VM has 512 MiB
    # Issue 075: the holder of a line or device is the driver that uses it, not init, which keeps a copy for restarts.
    pids = vm.services()
    irqs = vm.command("irqs", raw=True)
    assert re.search(fr"^IRQ=1 COUNT=\d+ HOLDER={pids['ps2_kbd']} HOLDERS=2 ", irqs, re.M), irqs
    assert re.search(r"^\d+ 00:01\.1 010180 IDE controller IRQ=", vm.command("devices"), re.M)
    endpoints = vm.command("endpoints", raw=True)
    assert len(re.findall(r"^EP=\d+ CREATOR=1 RECEIVERS=1 ", endpoints, re.M)) >= 6, endpoints
    for name in ("rtc", "vfs_server", "loader", "sysmon", "logd"):
        assert re.search(fr" SERVER={pids[name]} HOLDERS=\d+ IRQ=0$", endpoints, re.M), (name, endpoints)
    caps = vm.command(f"stat caps {pids['shell']}", raw=True)
    assert re.search(r"^SLOT=3 GEN=0 KIND=1 RIGHTS=\d+ SIZE=0 BADGE=\d+ EP=[1-9]\d* ", caps, re.M), caps  # the VFS client
    require(vm.service_logs("sysmon", "[SYSMON] READY"), "[SYSMON] READY: SAMPLES EVERY 100 MS")
    # Calendar date from the rtc service (idl/rtc.wit 1.1): QEMU's RTC follows the host's local time here.
    import datetime
    today = datetime.date.today()
    date = vm.command("date")
    assert any(f"DATE: {d.isoformat()} " in date for d in (today, today - datetime.timedelta(days=1), today + datetime.timedelta(days=1))), date
    require(vm.command("fg -4"), "ERROR:")  # the harness does not translate negative numbers
    vm.send("fg 0\n"); vm.expect("ERROR: EXPECTED ONE POSITIVE PID\nMIND> ")  # the whole reply, prompt included
    # End of the initial distribution: init gives up the platform privilege before READY.
    require(vm.service_logs("init", "[INIT] READY"), "[INIT] PLATFORM PRIVILEGE DROPPED")
    # STAT: numbers agree with ps and heap; the shell's address space has its known layout; every CPU accounts time.
    tasks = vm.command("stat tasks", raw=True)
    assert int(re.search(r"STAT TASKS VERSION=2 COUNT=(\d+)", tasks)[1]) == len(re.findall(r"^\d+ [\w-]+ [A-Z_]+ (?:BG|FG) ", vm.command("ps", raw=True), re.M)), tasks
    # Issue 075: every task's kernel memory (context, mailbox, info and exit pages, page tables); the shell has the focus.
    assert all(int(k) >= 4 * 4096 for k in re.findall(r" KERNEL=(\d+)", tasks)) and re.search(r"^\d+ PARENT=\d+ shell .* FOCUS$", tasks, re.M), tasks
    stat_used = int(re.search(r"ARENA=67108864 USED=(\d+)", vm.command("stat memory", raw=True))[1])
    heap_now = int(re.search(r"HEAP: USED=(\d+)", vm.command("heap", raw=True))[1])
    assert abs(stat_used - heap_now) < 256 * 1024, (stat_used, heap_now)
    layout = vm.command(f"stat vmap {vm.services()['shell']}", raw=True)
    for region in ("IMAGE R-X", "STACK RW-", "SCREEN RW-", "INFO R--", "MAILBOX RW-", "0x8001000000 4096 GUARD ---"):
        require(layout, region)
    cpus = re.findall(r"CPU \d+ APIC=\d+ ONLINE=1 BUSY_MS=(\d+) IDLE_MS=(\d+)", vm.command("stat cpus", raw=True))
    assert len(cpus) == vm.cpus and all(int(b) + int(i) > 0 for b, i in cpus), cpus
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
    # In front, pong shows the last string it read and how many calls it answered (issue 098).
    vm.send("fg 1\n")
    vm.expect("FOREGROUND PID=1")
    time.sleep(.3)
    screen = screen_text(vm)
    vm.serial()
    assert any(canon("HELLO FROM PING! ZERO-COPY IPC SUCCESS! COUNT: 100") in row for row in screen), screen
    assert any(re.search(canon("CALLS ANSWERED: ") + r"[1-9]", row) for row in screen) and not any(canon("WAITING") in row for row in screen), screen
    vm.background(1)
    # Killing a server whose client awaits a reply wakes the client with an error instead of hanging the kernel.
    require(vm.command("kill 1"), "KILLED PID=1")
    time.sleep(.5)
    assert 2 in task_rows(vm), "client of a dead server must survive"
    require(vm.command("kill 2"), "KILLED PID=2")
    # VFS: list the root and read files from the ATA disk via ata -> vfs_server.
    require(vm.service_logs("vfs_server", "[VFS] MOUNTED FAT16 FROM ATA"), "[VFS] MOUNTED FAT16 FROM ATA")
    require(vm.command("run files &"), "PID=3 NAME=files BACKGROUND")
    files_check(vm, 3)
    # Cyrillic, an em dash and box drawing in the 8x16 font (MIND Mono 16), checked pixel for pixel.
    vm.send("fg 3\n")
    vm.expect("FOREGROUND PID=3")
    time.sleep(.2)
    check_text16(vm, 24, 24, "Files — демо VFS-сервера ╞═╡ Esc: выход", 0x80D0FF, 0x101820)
    vm.serial(); vm.background(3)
    require(vm.command("kill 3"), "KILLED PID=3")
    # loader: programs are read from disk, not the kernel table — new files launch too.
    listing = vm.command("list")
    for name in ("clock", "dzen-clock", "hello", "files"):
        require(listing, f"  {name} ")
    assert "kernel " not in listing
    # Sorted down the columns and short enough for the screen (one per line, fm scrolled off the top); the services
    # on their own line; -l says what each program does.
    rows = [line for line in listing.splitlines() if line.startswith("  ")]
    count = int(re.search(r"PROGRAMS ON DISK \((\d+)\)", listing)[1])
    names = [name for row in rows for name in row.split()]
    assert len(names) == count and "fm" in names and "vfs_server" not in names and len(rows) < 20, listing
    assert sorted(names) == [rows[r].split()[c] for c in range(len(rows[0].split())) for r in range(len(rows)) if c < len(rows[r].split())], listing
    require(listing, "SERVICES (STARTED AT BOOT; SVC SHOWS THEIR STATE): init logd rtc")
    # Every program says what it does (mind::about!, issue 091): list -l takes the first line from the program file.
    detailed = vm.command("list -l")
    described = re.findall(r"^  ([\w-]+) +\d+ KB  (.*)$", detailed, re.M)
    assert len(described) == count and all(text.strip() for _, text in described), detailed
    assert re.search(r"^  fm +\d+ KB  file manager \(Norton Commander keys\)", detailed, re.M), detailed
    assert re.search(r"^  hello +\d+ KB  clock — a digital clock", detailed, re.M), detailed  # a copy of clock.elf
    # A mask keeps the names that match: list a* shows the programs (and services) starting with a.
    output = vm.command("list a*")
    matching = int(re.search(r"PROGRAMS ON DISK MATCHING a\* \((\d+)\):", output)[1])
    names = [name for row in output.splitlines() if row.startswith("  ") for name in row.split()]
    assert len(names) == matching and {"app", "app2"} <= set(names) and all(n.startswith("a") for n in names), output
    require(output, "SERVICES (STARTED AT BOOT; SVC SHOWS THEIR STATE): ata ahci audio_gw\n")
    output = vm.command("list -l f*")
    assert re.search(r"^  fm +\d+ KB  file manager", output, re.M) and not re.search(r"^  [^f]", output, re.M), output
    assert "SERVICES" not in output, output
    require(vm.command("list zz*"), "PROGRAMS ON DISK MATCHING zz* (0).")
    require(vm.command("list -x"), "USAGE: LIST [-L] [MASK]")
    require(vm.command("run hello &"), "PID=4 NAME=hello BACKGROUND")
    # The address space of a known program (hello is clock.elf) as STAT_VMAP reports it: the layout paging.rs sets up.
    pmap = vm.command("pmap 4")
    require(pmap, "0x0000008000000000 ")
    assert re.search(r"0x0000008000000000 +\d+ r-x image", pmap), pmap
    for line in ("0x0000008001001000     65536 rw- stack",
                 "0x0000008004000000      4096 r-- info", "0x0000008004001000      4096 rw- mailbox", "0x0000008005000000      4096 r-x exit"):
        require(pmap, line)
    assert re.search(r"0x0000008002000000 +\d+ rw- screen", pmap), pmap
    details = vm.command("stat 4")
    require(details, "NAME=hello")
    require(details, "QUOTA TASKS=0/0 ENDPOINTS=0/4")
    require(details, "CAPS=5/95")
    # The standard client endpoints in slots 2..6, write and grant only; no privilege.
    caps = vm.command("caps 4")
    for slot in (2, 3, 4, 5, 6):
        assert re.search(fr"SLOT={slot} GEN=0 endpoint NODE=\d+ PARENT=\d+ EP=[1-9]\d* RIGHTS=-wg-", caps), (slot, caps)  # EP: issue 075
    assert not re.search(r"(control|platform|spawn|observe|input|display)", caps), caps
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
    # Its client is not left waiting in a send to the dead endpoint (init keeps no receive right).
    time.sleep(.5)
    assert not re.search(r" hello WAIT=1:", vm.command("stat tasks", raw=True)), "client blocked on a quarantined service"
    require(vm.command("run rtc &"), "NAME=rtc")
    clock_resumes()
    vm.command("kill 6")
    # Loader v1 (idl/loader.wit): uptime asks in its ELF for the console and sysmon (idl/sysinfo.wit); the shell grants the
    # client endpoint in a launch session and shows the output of the console program (sysmon's last sample, taken
    # every 100 ms, may already count it as a task).
    time.sleep(1.2)
    tasks = len(re.findall(r"^\d+ [\w-]+ ", vm.command("ps", raw=True), re.M))
    uptime = vm.command("uptime")
    require(uptime, "NAME=uptime FOREGROUND")
    match = re.search(r"^up \d+:\d\d:\d\d, load \d+\.\d\d \d+\.\d\d \d+\.\d\d, cpu (\d+)%, (\d+) tasks$", uptime, re.M)
    assert match and int(match[2]) in (tasks, tasks + 1) and 0 <= int(match[1]) <= 100, (uptime, tasks)
    # In the background: the output of the exited program stays readable.
    pid = int(re.search(r"PID=(\d+) NAME=uptime BACKGROUND", vm.command("run uptime &"))[1])
    time.sleep(.5)
    assert pid not in task_rows(vm)
    require(vm.command(f"logs {pid}"), " tasks")
    assert "FAULT PID=" not in vm.command("faults")
    dmesg_check(vm)
    lifecycle_check(vm)
    # help <name>: the program's text read from its file, the shell's own lines, or a service; nothing is started.
    output = vm.command("help fm")
    require(output, "Usage: fm [directory]")
    assert "STARTED" not in output, output
    output = vm.command("fm --help")  # a program with a screen: the shell shows its text instead of starting it
    require(output, "fm — file manager")
    assert "STARTED" not in output, output
    require(vm.command("help cat"), "- ls [path], cat <file>: files")
    output = vm.command("help voice")
    require(output, "- voice on [--wav file] [seconds], voice off, voice listen: voice control")
    require(output, "PROGRAM voice:")
    require(vm.command("help rtc"), "rtc — a service init starts at boot")
    require(vm.command("help nosuch"), "ERROR: NO COMMAND OR PROGRAM CALLED nosuch.")
    # A console program answers --help itself, into the shell; for one with a screen (whose output would leave with it)
    # the shell shows the same text from its file, with or without `run`.
    output = vm.command("uptime --help")
    require(output, "NAME=uptime FOREGROUND")
    require(output, "uptime — uptime, load averages")
    output = vm.command("run view --help")
    require(output, "view — text and hex viewer.")
    assert "STARTED" not in output, output
    # Final recovery boundary: without init the system stops instead of running unsupervised.
    vm.send(f"kill {vm.services()['init']}\n", raw=True)
    vm.expect("INIT EXITED: SYSTEM HALTED")
    print("PASS: boot services, monotonic clock, single instances, IPC call/reply with memory caps, peer death, VFS list/read over ATA driver + FAT, programs loaded from disk by loader, supervised restart with budget and quarantine for existing clients, launch sessions with requested capabilities, console programs, the system log, lifecycle control (svc, top), help and --help from the programs' files, halt without init, reclaim", flush=True)


def lifecycle_check(vm):
    """init's lifecycle requests (idl/init.wit 1.1): svc lists, restarts, stops and starts services, refuses init and
    the shell; clients reach a restarted service; top stops an application after asking."""
    import datetime
    pids = vm.services()
    listing = vm.command("svc", raw=True)
    require(listing, "SERVICE         PID  STARTS  STATE    HOLDS")
    assert re.search(r"^init\s+1\s+1\s+running\s+restart and process control$", listing, re.M), listing
    assert re.search(fr"^logd\s+{pids['logd']}\s+1\s+running\s+observe privilege$", listing, re.M), listing
    assert re.search(r"^ahci\s+0\s+0\s+stopped\s+AHCI registers", listing, re.M), listing
    # A restarted rtc: a new PID, one more start, and the shell's client reaches it.
    starts = int(re.search(r"^rtc\s+\d+\s+(\d+)", listing, re.M)[1])
    # (the harness shows new PIDs as ordinals: add BASE for the real one)
    new = int(re.search(r"rtc restarted: PID (\d+)", vm.command("svc restart rtc"))[1]) + BASE
    assert new != pids["rtc"] and new == vm.services()["rtc"], (new, pids)
    assert re.search(fr"^rtc\s+{new}\s+{starts + 1}\s+running", vm.command("svc", raw=True), re.M)
    today = datetime.date.today()
    date = vm.command("date")
    assert any(f"DATE: {d.isoformat()} " in date for d in (today, today - datetime.timedelta(days=1), today + datetime.timedelta(days=1))), date
    # Stop and start; what may not be stopped; a missing device; usage.
    require(vm.command("svc stop shell"), "svc: stop shell: init and the shell cannot be stopped")
    require(vm.command("svc stop init"), "svc: stop init: init and the shell cannot be stopped")
    require(vm.command("svc stop tts"), "tts stopped")
    assert "tts" not in vm.services()
    require(vm.command("svc stop tts"), "svc: stop tts: it does not run")
    require(vm.command("svc start tts"), "tts started: PID")
    require(vm.command("svc start tts"), "svc: start tts: it runs already")
    require(vm.command("svc start ahci"), "svc: start ahci: no such service, or its device is missing")
    require(vm.command("svc restart nothing"), "svc: restart nothing: no such service or task")
    require(vm.command("svc stop 1"), "svc: stop 1: init and the shell cannot be stopped")
    require(vm.command("svc frobnicate"), "usage: svc")
    # An application stopped by PID, and one stopped from top (k, then Stop).
    first = int(re.search(r"PID=(\d+) NAME=clock BACKGROUND", vm.command("run clock &"))[1]) + BASE
    require(vm.command(f"svc stop {first}", raw=True), f"{first} stopped")
    assert first - BASE not in task_rows(vm)
    clock = int(re.search(r"PID=(\d+) NAME=clock BACKGROUND", vm.command("run clock &"))[1]) + BASE
    vm.send("top\n")
    vm.expect("[TOP] READY")
    vm.send("N")
    vm.expect("[TOP] SORT=PID")
    vm.send_bytes(b"\x1b[F")  # End: top itself, the newest task
    status_line(vm, "SORT=PID", raw=True)
    vm.send_bytes(b"\x1b[A")
    assert f"SELECTED={clock} " in status_line(vm, "SORT=PID", raw=True)
    vm.send("k")
    assert "CONFIRM=STOP" in status_line(vm, "SORT=PID", raw=True)
    vm.send_bytes(b"\x1b[D")
    status_line(vm, "CONFIRM=STOP", raw=True)
    vm.send_bytes(b"\r")
    assert "CONFIRM=NONE" in status_line(vm, "SORT=PID", raw=True)
    vm.send("q")
    require(vm.expect("EXITED. SHELL RESUMED."), "[TOP] DONE")
    time.sleep(.1); vm.collect(); vm.output = ""
    assert not re.search(fr"^{clock} clock ", vm.command("ps", raw=True), re.M)
    require(vm.command("dmesg -s init"), f"[INIT] STOPPED rtc PID={pids['rtc'] - BASE}")


def dmesg_check(vm):
    """logd (idl/log.wit) and dmesg: boot lines of init and the services with the sources logd stamped; a line that
    names another source keeps its real one; filters; reading needs the shell's client."""
    pids = vm.services()
    output = vm.command("dmesg", raw=True)
    require(output, f"logd({pids['logd']}) [LOGD] READY: 256 RECORDS OF 200 BYTES")
    require(output, f"init({pids['init']}) [INIT] STARTED logd PID={pids['logd']}")  # printed before logd ran: kept, then sent
    require(output, f"init({pids['init']}) [INIT] STARTED vfs_server PID={pids['vfs_server']}")
    require(output, f"vfs_server({pids['vfs_server']}) [VFS] MOUNTED FAT")
    require(output, f"loader({pids['loader']}) [LOADER] READY")
    assert re.search(r"^\[\s*\d+\.\d{3}\] ", output, re.M), output
    # The text may claim any source; logd records the sender.
    require(vm.command(f"logger vfs_server({pids['vfs_server']}) [VFS] FORGED LINE"), "LOGGED")
    require(vm.command("dmesg -n 1", raw=True), f"shell({pids['shell']}) vfs_server({pids['vfs_server']}) [VFS] FORGED LINE")
    only = vm.command("dmesg -s vfs_server", raw=True)
    assert "FORGED" not in only and "[VFS] MOUNTED" in only and "[INIT]" not in only, only
    assert "[LOADER]" in vm.command(f"dmesg -s {pids['loader']}", raw=True)
    assert "[INIT]" not in vm.command("dmesg -l warn", raw=True)
    require(vm.command("dmesg -x"), "dmesg: usage: dmesg [-f] [-l level] [-s name|pid] [-n count]")
    # Follow mode in the background: a new line reaches it.
    follower = int(re.search(r"PID=(\d+) NAME=dmesg BACKGROUND", vm.command("run dmesg -f -n 0 &"))[1])
    time.sleep(.5)
    require(vm.command("logger FOLLOWED LINE"), "LOGGED")
    output = ""
    for _ in range(20):
        output += vm.command(f"logs {follower}")
        if "FOLLOWED LINE" in output:
            break
        time.sleep(.2)
    require(output, f"shell({pids['shell']}) FOLLOWED LINE")
    require(vm.command(f"kill {follower}"), "KILLED")


BLOCK_IMAGE_MB, BLOCK_FS_MB = 64, 60


def block_pattern(kind, sector):
    # tests/block_app.rs: pattern(kind, s)
    data = bytearray((j ^ sector * 31 ^ kind * 77) & 0xFF for j in range(512))
    header = b"MIND BLOCK WRITE TEST KIND=" + bytes([ord("0") + kind, ord(" "), ord("0") + sector])
    data[:len(header)] = header
    return bytes(data)


MTOOLS_ENV = dict(os.environ, MTOOLS_SKIP_CHECK="1")


def raw_fat_image(temp, replace=None):
    """A raw disk image: an MBR with one EFI system partition (what OVMF boots from a fixed disk) holding a FAT16 file
    system of BLOCK_FS_MB with the built OS; `replace` maps file names to other files. Returns (image, start, sectors)."""
    image = temp / "disk.img"
    start, fs_sectors = 2048, (BLOCK_FS_MB << 20) // 512
    with image.open("wb") as f:
        f.truncate(BLOCK_IMAGE_MB << 20)
        mbr = bytearray(512)
        mbr[446:462] = struct.pack("<B3sB3sII", 0x80, b"\xfe\xff\xff", 0xEF, b"\xfe\xff\xff", start, fs_sectors)
        mbr[510:512] = b"\x55\xaa"
        f.write(mbr)
    subprocess.run(["mkfs.fat", "-F", "16", "-n", "MINDTEST", "--offset", str(start), "-h", str(start), str(image), str(BLOCK_FS_MB << 10)], check=True, capture_output=True)
    files = temp / "files"
    (files / "EFI/BOOT").mkdir(parents=True)
    for name in [*(p.name for p in (ROOT / "usb_root").glob("*.elf")), "EFI/BOOT/BOOTX64.EFI"]:
        shutil.copyfile(ROOT / "usb_root" / name, files / name)
    for name, source in (replace or {}).items():
        shutil.copyfile(source, files / name)
    subprocess.run(["mcopy", "-s", "-i", f"{image}@@{start * 512}", *[str(p) for p in files.iterdir()], "::"], check=True, env=MTOOLS_ENV, capture_output=True)
    return image, start, fs_sectors


def fsck_volume(image, start, fs_sectors):
    volume = image.parent / "volume.img"
    with image.open("rb") as f:
        f.seek(start * 512)
        volume.write_bytes(f.read(fs_sectors * 512))
    check = subprocess.run(["fsck.fat", "-n", str(volume)], capture_output=True, text=True)
    assert check.returncode == 0 and "Dirty bit" not in check.stdout + check.stderr, check.stdout + check.stderr
    volume.unlink()


def raw_tools():
    return all(shutil.which(t) for t in ("mkfs.fat", "mcopy", "mtype", "mdel", "fsck.fat"))


def vfs_suite(args):
    """Writing a raw FAT disk through vfs_server: the shell changes files in data/, syncs, the host checks the image
    (fsck.fat, mtools), and after a reboot the files are there while the RAM disk is empty again."""
    if not raw_tools():
        print("SKIP: vfs suite needs mkfs.fat, fsck.fat and mtools", flush=True)
        return
    with tempfile.TemporaryDirectory(prefix="smoke-vfs-", dir=ROOT / "usb_root") as temp:
        image, start, fs_sectors = raw_fat_image(Path(temp))
        part = f"{image}@@{start * 512}"
        vm = VM(args, image.relative_to(ROOT).as_posix(), raw=True, snapshot=False)
        try:
            mounted = vm.service_logs("vfs_server", "AS RAM:")  # reading drains the log: both lines at once
            require(mounted, "[VFS] MOUNTED FAT16 FROM ATA AT LBA 2048 (DEVICE WRITABLE)")
            require(mounted, "[VFS] MOUNTED FAT16 FROM RAM AS RAM: (")
            for command, answer in [("mkdir data/sub", "OK"), ("write data/notes.txt line one", "WROTE 9 BYTES"), ("write data/sub/a.txt alpha", "WROTE 6 BYTES"),
                                    ("mv data/sub/a.txt data/b.txt", "OK"), ("rm data/sub", "OK"), ("write ram:temp.txt scratch", "WROTE 8 BYTES"), ("sync", "OK")]:
                require(vm.command(command), answer)
            # screenshot (issue 086): the screen in front (the shell's) as a BMP in data/, and under a free name on ram:.
            def slow(command):  # 3 MB written through the ATA driver take a while under emulation
                vm.send(command + "\n")
                return vm.expect("MIND> ", timeout=180, after=command + "\n")
            vm.command("clear")  # a screen with room: the output after the capture must not scroll it
            shot = re.search(r"SCREENSHOT data/screen.bmp: (\d+)x(\d+), (\d+) BYTES", slow("screenshot data/screen.bmp"))
            assert shot, vm.log[-2000:]
            screen = vm.screenshot()
            vm.serial()
            require(slow("screenshot"), "SCREENSHOT ram:screen-001.bmp")
            require(slow("screenshot"), "SCREENSHOT ram:screen-002.bmp")
            require(vm.command("screenshot two words"), "USAGE: SCREENSHOT [FILE]")
        finally:
            vm.close()
            (Path(tempfile.gettempdir()) / f"mind-core-vfs-1-{args.cpus}cpu.log").write_text(vm.log)
        fsck_volume(image, start, fs_sectors)
        mtype = lambda name: subprocess.run(["mtype", "-i", part, f"::/{name}"], env=MTOOLS_ENV, capture_output=True).stdout
        assert mtype("data/notes.txt") == b"line one\n", mtype("data/notes.txt")
        assert mtype("data/b.txt") == b"alpha\n"
        assert b"sub" not in subprocess.run(["mdir", "-b", "-i", part, "::/data"], env=MTOOLS_ENV, capture_output=True).stdout
        # The BMP shows what QEMU's display showed: the command's own line (above the cursor's rows, which moved on).
        width, height, size = map(int, shot.groups())
        bmp = mtype("data/screen.bmp")
        assert len(bmp) == size and bmp[:2] == b"BM" and struct.unpack_from("<iiHH", bmp, 18) == (width, height, 1, 24), (len(bmp), bmp[:54])
        header, dims, pixels = screen.split(b"\n", 3)[0], screen.split(b"\n", 3)[1], screen.split(b"\n", 3)[3]
        assert header == b"P6" and tuple(map(int, dims.split())) == (width, height), (header, dims)
        stride = (width * 3 + 3) & ~3
        rows = [bmp[54 + (height - 1 - y) * stride:][:width * 3] for y in range(14)]
        assert len({row[i:i + 3] for row in rows for i in range(0, width * 3, 3)}) >= 2, "the line has text"
        for y, row in enumerate(rows):
            assert row == b"".join(pixels[(y * width + x) * 3:(y * width + x) * 3 + 3][::-1] for x in range(width)), y
        # After a reboot: the disk keeps its files, the RAM disk starts empty.
        subprocess.run(["mdel", "-i", part, "::/NvVars"], env=MTOOLS_ENV, capture_output=True)
        vm = VM(args, image.relative_to(ROOT).as_posix(), raw=True, snapshot=False)
        try:
            require(vm.command("cat data/notes.txt"), "line one")
            require(vm.command("ls data"), f"3 ENTRIES, 3 FILES, {15 + size} BYTES")  # with the screenshot
            require(vm.command("ls ram:"), "0 ENTRIES")
        finally:
            vm.close()
            (Path(tempfile.gettempdir()) / f"mind-core-vfs-2-{args.cpus}cpu.log").write_text(vm.log)
        fsck_volume(image, start, fs_sectors)
        # reboot (issue 084): an unsynced write reaches the disk, the services stop in reverse start order, the machine
        # boots again; `reboot -f` skips stopping them.
        vm = VM(args, image.relative_to(ROOT).as_posix(), raw=True, snapshot=False, reboot=True)
        try:
            require(vm.command("write data/reboot.txt kept"), "WROTE 5 BYTES")
            vm.send("reboot\n")
            output = vm.expect("MIND CORE KERNEL: REBOOT VIA", timeout=90)
            stopped = re.findall(r"^STOPPED (\w+)$", output, re.M)
            assert "REBOOTING..." in output and stopped[-1] == "logd" and "init" not in stopped and "shell" not in stopped, output
            assert stopped.index("vfs_server") < stopped.index("ata") < stopped.index("compositor"), stopped
            vm.expect("MIND CORE KERNEL: INIT STARTED", timeout=90)
            vm.expect("MIND> ", timeout=60)
            require(vm.command("cat data/reboot.txt"), "kept")
            vm.send("reboot -f\n")
            output = vm.expect("MIND CORE KERNEL: REBOOT VIA", timeout=60)
            assert "REBOOTING..." in output and "STOPPED" not in output, output
            vm.expect("MIND CORE KERNEL: INIT STARTED", timeout=90)
            vm.expect("MIND> ", timeout=60)
            require(vm.command("reboot now"), "USAGE: REBOOT [-F]")
        finally:
            vm.close()
            (Path(tempfile.gettempdir()) / f"mind-core-vfs-3-{args.cpus}cpu.log").write_text(vm.log)
        fsck_volume(image, start, fs_sectors)
    print("PASS: vfs: files written to a raw FAT disk in data/ pass fsck.fat and read back with mtools and after a reboot; the RAM disk is empty after it; "
          f"screenshot writes the screen as a BMP ({width}x{height}); reboot stops {len(stopped)} services and keeps an unsynced file; reboot -f", flush=True)


def edit_check(vm):
    """The editor on the raw disk: new files on ram: and in data/ typed in Latin and Cyrillic, saved with F2 and from
    the unsaved-changes dialog, read back; a CRLF file; a boot file opens read-only and is saved elsewhere."""
    def keys(data, text):
        vm.send_bytes(data)
        return status_line(vm, text)
    def utf8(line):
        vm.send_bytes((line + "\n").encode())
        return vm.expect("MIND> ")
    def start(path):
        vm.send_bytes(f"edit {path}\n".encode())
        return status_line(vm, "[EDIT] READY")
    def leave(data, text):
        vm.send_bytes(data)
        require(vm.expect("EXITED. SHELL RESUMED."), text)
        time.sleep(.1); vm.collect(); vm.output = ""
    f2, shift_f2, f10, ctrl_end = b"\x1bOQ", b"\x1b[12;2~", b"\x1b[21~", b"\x1b[1;5F"
    # A new file on the RAM disk.
    line = start("ram:hello.txt")
    assert "LINE=1 COL=1 BYTES=0 LINES=1 MODIFIED=0 DIALOG=NONE MENU=0 RO=0" in line, line
    keys("Hello, ".encode(), "BYTES=7 ")
    keys("мир".encode(), "BYTES=13 ")
    keys(b"\r", "LINE=2 COL=1 BYTES=14 ")
    keys("строка 2".encode(), "LINE=2 COL=9 BYTES=28 LINES=2 MODIFIED=1")
    # The keys (F1) open over the text; the Enter that leaving the QEMU monitor sends closes them.
    keys(b"\x1bOP", "DIALOG=HELP")
    time.sleep(.3)
    screen = screen_text(vm)
    vm.serial()
    keys(b"\x1b[15~", "DIALOG=NONE")  # F5 is bound to nothing: a fresh state line (serial() may drop the last one)
    assert canon("ram:hello.txt") in screen[0] and canon("Ln 2 Col 9") in screen[0] and canon("INS") in screen[0], screen[0]
    assert screen[1].startswith(canon("Hello, мир")) and screen[2].startswith(canon("строка 2")), screen[1:3]
    assert canon("2Save") in screen[-1] and canon("10Quit") in screen[-1], screen[-1]
    assert any(canon("Ctrl+U or Alt+Backspace: undo") in row for row in screen), screen
    keys(f2, "[EDIT] SAVED 28 BYTES TO ram:hello.txt")
    leave(f10, "[EDIT] DONE")
    output = utf8("cat ram:hello.txt")
    require(output, "Hello, мир")
    require(output, "строка 2")
    # Reopened: a change, then F10 and Save in the dialog.
    assert "BYTES=28 LINES=2 MODIFIED=0" in start("ram:hello.txt")
    keys(ctrl_end, "LINE=2 COL=9 ")
    keys(b"!", "BYTES=29 ")
    keys(f10, "DIALOG=UNSAVED")
    leave(b"\r", "[EDIT] SAVED 29 BYTES TO ram:hello.txt")
    require(utf8("cat ram:hello.txt"), "строка 2!")
    # A new file in data/ of the boot disk.
    assert "BYTES=0 " in start("data/edit.txt")
    keys(b"Line one", "BYTES=8 ")
    keys(b"\r", "BYTES=9 ")
    keys("Вторая строка".encode(), "BYTES=34 ")
    keys(b"\r", "LINE=3 COL=1 BYTES=35 ")
    keys(f2, "[EDIT] SAVED 35 BYTES TO data/edit.txt")
    # The editor's client is confined to data/: other places are refused (issue 071).
    for outside in (b"ram:x.txt", b"kernel.elf", b"EFI/x.txt"):
        keys(shift_f2, "DIALOG=SAVEAS")
        keys(b"\x7f" * 16, "DIALOG=SAVEAS")
        keys(outside, "DIALOG=SAVEAS")
        keys(b"\r", "[EDIT] NOT SAVED denied: the editor may change only its file's directory")
    leave(f10, "[EDIT] DONE")
    # A file with CR LF line endings keeps them; a new line gets one too.
    assert "BYTES=15 LINES=3 MODIFIED=0" in start("data/crlf.txt")
    keys(ctrl_end, "LINE=3 COL=1 ")
    keys("три".encode(), "BYTES=21 ")
    keys(b"\r", "LINE=4 COL=1 BYTES=23 ")
    keys(b"4", "BYTES=24 ")
    keys(f2, "[EDIT] SAVED 24 BYTES TO data/crlf.txt")
    leave(f10, "[EDIT] DONE")
    # A boot file opens read-only (its directory is read-only to the editor): typing changes nothing, and a copy
    # cannot go elsewhere.
    assert "BYTES=33 LINES=2 MODIFIED=0 DIALOG=NONE MENU=0 RO=1" in start("readme.txt")
    keys(b"x", "BYTES=33 LINES=2 MODIFIED=0")
    keys(shift_f2, "DIALOG=SAVEAS")
    keys(b"\x7f" * 10, "DIALOG=SAVEAS")
    keys(b"ram:copy.txt", "DIALOG=SAVEAS")
    keys(b"\r", "[EDIT] NOT SAVED denied")
    leave(f10, "[EDIT] DONE")
    require(vm.command("ls ram:"), "1 ENTRIES, 1 FILES")
    require(vm.command("ls data"), "2 ENTRIES, 2 FILES, 59 BYTES")
    # vfs_server made a scope for each start and ended those whose editor had exited.
    scopes = vm.command("dmesg -s vfs_server")
    for made in ("FOR ram:/ (WRITABLE)", "FOR :/data (WRITABLE)", "FOR :/ (READ-ONLY)"):
        require(scopes, made)
    require(scopes, "ENDED")
    print("PASS: edit: Latin and Cyrillic text saved on ram: and in data/ (F2, the unsaved-changes dialog), read back; CRLF kept; a boot file opens read-only; the editor's client is confined to its file's directory", flush=True)


def edit_suite(args):
    """The editor on a raw FAT disk; the host then checks the image with fsck.fat and reads the files with mtools."""
    if not raw_tools():
        print("SKIP: edit suite needs mkfs.fat, fsck.fat and mtools", flush=True)
        return
    with tempfile.TemporaryDirectory(prefix="smoke-edit-", dir=ROOT / "usb_root") as temp:
        readme, crlf = Path(temp) / "readme.txt", Path(temp) / "crlf.txt"
        readme.write_bytes("Только для чтения\n".encode())
        crlf.write_bytes("один\r\ntwo\r\n".encode())
        image, start, fs_sectors = raw_fat_image(Path(temp), {"readme.txt": readme})
        part = f"{image}@@{start * 512}"
        mtools = lambda *command: subprocess.run(list(command), env=MTOOLS_ENV, check=True, capture_output=True).stdout
        mtools("mmd", "-i", part, "::/data")
        mtools("mcopy", "-i", part, str(crlf), "::/data/crlf.txt")
        vm = VM(args, image.relative_to(ROOT).as_posix(), raw=True, snapshot=False)
        try:
            edit_check(vm)
        finally:
            vm.close()
            (Path(tempfile.gettempdir()) / f"mind-core-edit-{args.cpus}cpu.log").write_text(vm.log)
        fsck_volume(image, start, fs_sectors)
        assert mtools("mtype", "-i", part, "::/data/edit.txt") == "Line one\nВторая строка\n".encode()
        assert mtools("mtype", "-i", part, "::/data/crlf.txt") == "один\r\ntwo\r\nтри\r\n4".encode()
        assert mtools("mtype", "-i", part, "::/readme.txt") == readme.read_bytes()
        names = mtools("mdir", "-b", "-i", part, "::/data").decode().split()
        assert sorted(n.rsplit("/", 1)[-1] for n in names) == ["crlf.txt", "edit.txt"], names
    print("PASS: edit image: fsck.fat clean; mtools reads the saved files byte for byte; no temporary files left", flush=True)


def fat16_chain_link(image, start, name):
    """The FAT16 volume at sector `start` of `image`: (offsets of the FAT entry of the second cluster of root file
    `name` (8.3, upper case) in every FAT copy, its value)."""
    with image.open("rb") as f:
        f.seek(start * 512)
        boot = f.read(512)
        reserved, fats, root_entries, fat_size = struct.unpack_from("<H", boot, 14)[0], boot[16], struct.unpack_from("<H", boot, 17)[0], struct.unpack_from("<H", boot, 22)[0]
        f.seek((start + reserved + fats * fat_size) * 512)
        root = f.read(root_entries * 32)
        short = name.split(".")[0].ljust(8).encode() + name.split(".")[1].ljust(3).encode()
        first = next(struct.unpack_from("<H", root, i + 26)[0] for i in range(0, len(root), 32) if root[i:i + 11] == short)
        fat_at = lambda copy, cluster: (start + reserved + copy * fat_size) * 512 + cluster * 2
        f.seek(fat_at(0, first))
        second = struct.unpack("<H", f.read(2))[0]
        f.seek(fat_at(0, second))
        value = struct.unpack("<H", f.read(2))[0]
    return [fat_at(copy, second) for copy in range(fats)], value


def patch_words(image, offsets, value):
    with image.open("r+b") as f:
        for offset in offsets:
            f.seek(offset)
            f.write(struct.pack("<H", value))


def disk_check(vm):
    """fm writes (copy a tree from the boot disk to ram:, rename, copy back to data/, delete, mkdir, edit in place),
    df before and after, fsck on a volume with a broken chain and on a clean one."""
    def keys(data, text):
        vm.send_bytes(data)
        return status_line(vm, text)
    def leave(text):
        vm.send_bytes(b"\x1b[21~")
        require(vm.expect("EXITED. SHELL RESUMED."), text)
        time.sleep(.1); vm.collect(); vm.output = ""
    def ram_free():
        line = next(l for l in vm.command("df").splitlines() if l.startswith("ram:"))
        return int(line.split()[-2])
    f5, f6, f7, f8, alt_f2, end = b"\x1b[15~", b"\x1b[17~", b"\x1b[18~", b"\x1b[19~", b"\x1b[12;3~", b"\x1b[F"
    listing = vm.command("df")
    require(listing, "VOLUME  LABEL        TYPE   CLUSTER    SIZE KB    USED KB    FREE KB  USE")
    assert re.search(r"A:\s+MINDTEST\s+FAT16\s+\d+\s+\d+", listing), listing
    assert re.search(r"ram:\s+MIND RAM\s+FAT16\s+\d+\s+8\d{3}\s+0\s+8\d{3}\s+0%", listing), listing
    before = ram_free()
    # Session 1: data/tree copied to ram: with F5.
    vm.send("fm data\n")
    require(status_line(vm, "[FM] READY"), "LEFT=/data FULL RIGHT=/ BRIEF ACTIVE=L CURRENT=..")
    keys(alt_f2, "DIALOG=VOLUME")
    keys(b"\x1b[B", "DIALOG=VOLUME")
    keys(b"\r", "RIGHT=/ram: BRIEF")
    keys(b"\x1b[B", "CURRENT=tree ")
    keys(f5, "DIALOG=TARGET")
    vm.send_bytes(b"\r")
    require(status_line(vm, "DIALOG=NONE MENU=0 VIEW=0 JOB=NONE", whole=True), "RIGHT=/ram: BRIEF")
    leave("[FM] DONE")
    require(vm.command("ls ram:tree"), "2 ENTRIES, 1 FILES, 6 BYTES")
    require(vm.command("ls ram:tree/sub"), "1 ENTRIES, 1 FILES, 9 BYTES")
    copied = ram_free()
    assert copied < before, (before, copied)
    # Session 2: rename on ram:, copy back to data/, delete from ram:, a new directory, an edit in place.
    vm.send("fm data\n")
    status_line(vm, "[FM] READY")
    keys(alt_f2, "DIALOG=VOLUME")
    keys(b"\x1b[B", "DIALOG=VOLUME")
    keys(b"\r", "RIGHT=/ram: BRIEF")
    keys(b"\t", "ACTIVE=R CURRENT=tree ")
    keys(f6, "DIALOG=TARGET")
    keys(b"\x7f" * 24, "DIALOG=TARGET")
    keys(b"ram:/moved", "DIALOG=TARGET")
    vm.send_bytes(b"\r")
    require(status_line(vm, "DIALOG=NONE MENU=0 VIEW=0 JOB=NONE", whole=True), "CURRENT=moved ")
    keys(f5, "DIALOG=TARGET")  # to the other panel: A:/data
    vm.send_bytes(b"\r")
    require(status_line(vm, "DIALOG=NONE MENU=0 VIEW=0 JOB=NONE", whole=True), "ACTIVE=R")
    keys(f8, "DIALOG=DELETE")
    vm.send_bytes(b"\r")
    require(status_line(vm, "DIALOG=NONE MENU=0 VIEW=0 JOB=NONE", whole=True), "RIGHT=/ram: BRIEF")
    keys(f7, "DIALOG=MKDIR")
    keys("готово".encode(), "DIALOG=MKDIR")
    keys(b"\r", "CURRENT=готово ")
    keys(b"\t", "ACTIVE=L CURRENT=moved ")  # the copy put the cursor on it
    keys(b"\r", "LEFT=/data/moved FULL")
    keys(end, "CURRENT=a.txt ")
    keys(b"\x1bOS", "EDITOR LINE=1 COL=1 BYTES=6 LINES=2 MODIFIED=0")
    keys(b"\x1b[1;5F", "EDITOR LINE=2 COL=1 ")
    keys("ещё".encode(), "BYTES=12 ")
    keys(b"\x1bOQ", "MODIFIED=0")
    keys(b"\x1b[21~", "LEFT=/data/moved FULL")
    leave("[FM] DONE")
    require(vm.command("ls ram:"), "1 ENTRIES, 0 FILES, 0 BYTES")
    require(vm.command("ls data/moved"), "2 ENTRIES, 1 FILES, 12 BYTES")
    vm.send_bytes(b"cat data/moved/sub/b.txt\n")
    require(vm.expect("MIND> "), "бета")
    after = ram_free()
    assert copied < after <= before, (before, copied, after)  # only the new directory's cluster (512 bytes) is used
    # fsck: the boot disk has a broken chain (broken.txt), the RAM disk is clean; nothing is changed.
    output = vm.command("fsck")
    require(output, "ERRORS: lost clusters")
    require(output, "broken chains 1, size mismatches 1")
    require(output, "first: broken.txt: the cluster chain runs into a free or invalid cluster")
    assert re.search(r"ram: MIND RAM FAT16: 0 files, 1 directories; 1 clusters used, \d+ free\s+clean", output), output
    require(output, "fsck: 1 volume with errors (nothing was changed)")
    require(vm.command("fsck ram:"), "fsck: no errors (nothing was changed)")
    print("PASS: disk: fm copies a tree to ram:, renames, copies back to data/, deletes, makes a directory and edits in place; df follows; fsck finds a broken chain and passes a clean volume", flush=True)
    # format (issue 083): only ram:, only with -y; the files are gone, the label is new, the volume works at once.
    require(vm.command("write ram:keep.txt kept"), "WROTE")
    require(vm.command("format ram: -l scratch"), "FORMAT: THIS ERASES ALL FILES ON ram: (NOTHING WAS CHANGED). TO GO ON: format ram: -l scratch -y")
    require(vm.command("cat ram:keep.txt"), "kept")
    require(vm.command("format A:"), "FORMAT: ONLY THE RAM DISK (ram:) CAN BE FORMATTED, NOT A:")
    require(vm.command("format ram: -l scratch -y"), "FORMATTED ram: AS SCRATCH")
    require(vm.service_logs("vfs_server", "[VFS] FORMATTED RAM: AS SCRATCH"), "[VFS] FORMATTED RAM: AS SCRATCH (FAT16)")
    require(vm.command("ls ram:"), "0 ENTRIES")
    assert re.search(r"^ram: +SCRATCH +FAT16 ", vm.command("df"), re.M)
    require(vm.command("write ram:new.txt again"), "WROTE")
    require(vm.command("fsck ram:"), "fsck: no errors (nothing was changed)")
    require(vm.command("format ram: -y"), "FORMATTED ram: AS MIND RAM")
    print("PASS: format: ram: only, only with -y (without it nothing changes); a new label and an empty volume that works at once", flush=True)


def disk_suite(args):
    """fm's write operations, df and fsck on a raw FAT disk with a deliberately broken chain."""
    if not raw_tools():
        print("SKIP: disk suite needs mkfs.fat, fsck.fat and mtools", flush=True)
        return
    with tempfile.TemporaryDirectory(prefix="smoke-disk-", dir=ROOT / "usb_root") as temp:
        temp = Path(temp)
        (temp / "a.txt").write_text("alpha\n")
        (temp / "b.txt").write_bytes("бета\n".encode())
        (temp / "broken.txt").write_bytes(bytes(range(256)) * 64)  # 16 KiB: several clusters
        image, start, fs_sectors = raw_fat_image(temp, {"broken.txt": temp / "broken.txt"})
        part = f"{image}@@{start * 512}"
        mtools = lambda *command: subprocess.run(list(command), env=MTOOLS_ENV, check=True, capture_output=True).stdout
        for directory in ("data", "data/tree", "data/tree/sub"):
            mtools("mmd", "-i", part, f"::/{directory}")
        mtools("mcopy", "-i", part, str(temp / "a.txt"), "::/data/tree/a.txt")
        mtools("mcopy", "-i", part, str(temp / "b.txt"), "::/data/tree/sub/b.txt")
        # The second cluster of broken.txt is marked bad: the chain is broken and vfs_server never reuses the cluster.
        offsets, value = fat16_chain_link(image, start, "BROKEN.TXT")
        patch_words(image, offsets, 0xFFF7)
        vm = VM(args, image.relative_to(ROOT).as_posix(), raw=True, snapshot=False)
        try:
            disk_check(vm)
        finally:
            vm.close()
            (Path(tempfile.gettempdir()) / f"mind-core-disk-{args.cpus}cpu.log").write_text(vm.log)
        # fsck.fat agrees that the chain is broken; restored, the volume is clean.
        broken = image.parent / "broken.img"
        with image.open("rb") as f:
            f.seek(start * 512)
            broken.write_bytes(f.read(fs_sectors * 512))
        assert subprocess.run(["fsck.fat", "-n", str(broken)], capture_output=True).returncode != 0
        broken.unlink()
        patch_words(image, offsets, value)
        fsck_volume(image, start, fs_sectors)
        assert mtools("mtype", "-i", part, "::/data/moved/a.txt") == "alpha\nещё".encode()
        assert mtools("mtype", "-i", part, "::/data/moved/sub/b.txt") == "бета\n".encode()
        assert mtools("mtype", "-i", part, "::/broken.txt") == (temp / "broken.txt").read_bytes()
    print("PASS: disk image: with the chain restored, fsck.fat is clean; the files fm copied and edited read back with mtools", flush=True)


def block_suite(args, block_elf):
    """Block write through each driver: a raw FAT image (the file system in its first 60 MiB) boots with the test
    stand-in for vfs_server, which writes 8 sectors near the end of the disk through the write-badged client init
    gives it, flushes and reads them back; the image is then checked on the host."""
    if not raw_tools():
        print("SKIP: block suite needs mkfs.fat, fsck.fat and mtools", flush=True)
        return
    with tempfile.TemporaryDirectory(prefix="smoke-block-", dir=ROOT / "usb_root") as temp:
        image, start, fs_sectors = raw_fat_image(Path(temp), {"vfs_server.elf": block_elf})
        env = MTOOLS_ENV
        sectors = (BLOCK_IMAGE_MB << 20) // 512
        for kind, name, options in [(1, "ATA", {}), (2, "AHCI", {"ahci": True}), (3, "USB", {"usb": True})]:
            # OVMF keeps its variables (boot entries of the previous controller) in NvVars on the disk.
            subprocess.run(["mdel", "-i", f"{image}@@{start * 512}", "::/NvVars"], env=env, capture_output=True)
            vm = VM(args, image.relative_to(ROOT).as_posix(), raw=True, snapshot=False, **options)
            try:
                output = vm.service_logs("vfs_server", "[BLOCKTEST] DONE")
                line = next((l for l in output.splitlines() if f"KIND={kind} " in l), None)
                assert line and "BADGE=1 " in line and "WRITABLE" in line and f"SECTORS={sectors} " in line, output
                assert "ATTACH=OK WRITE=OK UNSEALED=REFUSED FLUSH=OK READBACK=OK" in line, line
            finally:
                vm.close()
                (Path(tempfile.gettempdir()) / f"mind-core-block-{name.lower()}-{args.cpus}cpu.log").write_text(vm.log)
            with image.open("rb") as f:
                f.seek((sectors - 8 * kind) * 512)
                written = f.read(8 * 512)
            assert written == b"".join(block_pattern(kind, s) for s in range(8)), f"{name}: the image does not hold the written sectors"
        fsck_volume(image, start, fs_sectors)
    print("PASS: block write: badged client of ATA, AHCI and USB drivers writes, flushes and reads back; the raw image holds the sectors; the file system is intact", flush=True)


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


SPEECH = ("открой файлы", "который час", "hello world")
# For `hear` (issue 078): two commands and a phrase outside the grammar.
COMMANDS = ("открой файлы", "what time is it", "сегодня хорошая погода")
# For voice control (issue 079): what is said at each push-to-talk, the answers to the shell's questions included.
DIALOGUE = ("открой файлы", "который час", "останови службу rtc", "нет", "останови службу rtc", "да", "прочитай заметки", "сегодня хорошая погода")


def speech_wav(phrases=SPEECH):
    """The phrases from the host build of tts, 1 s apart in faint noise, as 48 kHz stereo like the microphone gives;
    returns the WAV and where each phrase starts (ms)."""
    import random
    exe = Path(tempfile.gettempdir()) / "voice-tts-host"
    subprocess.run(["rustc", "--edition=2021", "-O", str(ROOT / "tests/tts_host.rs"), "-o", str(exe)], check=True)
    mono, starts = [0] * 8000, []
    for phrase in phrases:
        out = Path(tempfile.gettempdir()) / "voice-phrase.wav"
        subprocess.run([str(exe), phrase, str(out)], check=True, capture_output=True)
        data = out.read_bytes()[44:]
        starts.append(len(mono) // 16)
        mono += struct.unpack(f"<{len(data) // 2}h", data)
        mono += [0] * 16000
    noise = random.Random(77)
    mono = [max(-32768, min(32767, x + noise.randint(-30, 30))) for x in mono]
    frames, previous = [], 0
    for x in mono:  # 16 -> 48 kHz by linear interpolation, the same in both channels
        for value in ((2 * previous + x) // 3, (previous + 2 * x) // 3, x):
            frames += (value, value)
        previous = x
    pcm = struct.pack(f"<{len(frames)}h", *frames)
    header = b"RIFF" + struct.pack("<I", 36 + len(pcm)) + b"WAVEfmt " + struct.pack("<IHHIIHH", 16, 1, 2, 48000, 48000 * 4, 4, 16)
    return header + b"data" + struct.pack("<I", len(pcm)) + pcm, starts


def listen_suite(vm, starts):
    # The network card the launchers add shares the sound card's interrupt line: both drivers hear it (issue 159);
    # audio_gw would also play without interrupts, looking at the ring while a client waits (096).
    lines = dict(re.findall(r"(Ethernet|audio \(AC97\)) IRQ=(\d+)", vm.command("devices")))
    assert len(lines) == 2, lines
    shared = len(set(lines.values())) == 1
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
    if shared:
        # Both drivers are bound to the line and audio_gw got interrupts while say spoke (issue 159).
        line = lines["audio (AC97)"]
        assert re.search(fr"^IRQ={line} COUNT=\d+ .* ENDPOINTS=\d+,\d+", vm.command("irqs", raw=True), re.M), vm.command("irqs", raw=True)
        require(vm.service_logs("audio_gw", "[AUDIO] IRQ COUNT"), "[AUDIO] IRQ COUNT")
    assert 200 < spoken < 2000, spoken
    # Run by name: a plain word starts the program in the foreground with the rest as arguments.
    vm.send("say hi\n")
    output = vm.expect("PID=3 EXITED. SHELL RESUMED.", timeout=20)
    require(output, "STARTED PID=3 NAME=say FOREGROUND")
    spoken = int(re.search(r"\[SAY\] SPOKE (\d+) MS", output)[1])
    assert 50 < spoken < 1500, spoken
    require(vm.command("nosuchprogram"), "ERROR: UNKNOWN COMMAND")
    require(vm.command("run rtc x &"), "SERVICES TAKE NO ARGUMENTS")

    def logs(pid, end="[LISTEN] DONE"):
        text = ""
        for _ in range(120):
            text += vm.command(f"logs {pid}")
            if end in text:
                break
            time.sleep(.25)
        return text
    # Speech detection (mind::voice): nothing in the microphone's silence; the phrases of a WAV file made by the host
    # build of tts, each found once where it starts.
    require(vm.command("run listen --vad 1 &"), "PID=4 NAME=listen BACKGROUND")
    log = logs(4)
    require(log, "[LISTEN] LISTENING FOR SPEECH 1 S")
    require(log, "[LISTEN] 0 UTTERANCES IN 1000 MS")
    vm.command("kill 4")
    require(vm.command("run listen --vad --wav speech.wav &"), "PID=5 NAME=listen BACKGROUND")
    log = logs(5)
    require(log, "[LISTEN] FILE speech.wav: 48000 HZ, 2 CHANNELS")
    require(log, f"[LISTEN] {len(starts)} UTTERANCES IN")
    found = [int(ms) for ms in re.findall(r"\[LISTEN\] SPEECH AT (\d+) MS, (?:\d+) MS, LEVEL -\d+ DBFS", log)]
    assert len(found) == len(starts) and all(-60 <= f - s <= 60 for f, s in zip(found, starts)), (found, starts)
    vm.command("kill 5")
    # Without --vad the file is converted to 16 kHz mono, measured and played back.
    require(vm.command("run listen --wav speech.wav &"), "PID=6 NAME=listen BACKGROUND")
    log = logs(6)
    duration = re.search(r"\[LISTEN\] FILE speech.wav: 48000 HZ, 2 CHANNELS, (\d+) MS", log)[1]
    require(log, f"[LISTEN] 16 KHZ MONO: {duration} MS, PEAK")
    require(log, "[LISTEN] PLAYED BACK")
    vm.command("kill 6")
    require(vm.command("run listen --wav nosuch.wav &"), "PID=7 NAME=listen BACKGROUND")
    require(logs(7), "[LISTEN] CANNOT READ nosuch.wav: File(NotFound)")
    vm.command("kill 7")
    require(vm.command("run listen --wav &"), "PID=8 NAME=listen BACKGROUND")
    require(logs(8), "USAGE: LISTEN [SECONDS] | LISTEN --vad [SECONDS] | LISTEN [--vad] --wav FILE")
    vm.command("kill 8")
    # hear (issue 078): commands recognized in a WAV file, the phrase outside the grammar refused; the microphone's
    # silence holds nothing.
    vm.send("hear --wav commands.wav\n")
    heard = vm.expect("MIND> ", timeout=180, after="hear --wav commands.wav\n")
    require(heard, 'HEARD "открой файлы" INTENT=open TOOL=fm CONFIDENCE=')
    require(heard, 'HEARD "what time is it" INTENT=time CONFIDENCE=')
    require(heard, "NOT UNDERSTOOD (CLOSEST")
    assert heard.count("HEARD") == 2 and heard.count("NOT UNDERSTOOD") == 1, heard
    vm.send("hear 1\n")
    require(vm.expect("MIND> ", timeout=60, after="hear 1\n"), "NOTHING HEARD")
    require(vm.command("hear --wav nosuch.wav"), "HEAR: CANNOT READ nosuch.wav: File(NotFound)")
    require(vm.command("hear x y"), "USAGE: HEAR [SECONDS] | HEAR --wav FILE")
    # The microphone has one owner at a time (idl/audio.wit 1.1): hear cannot record while listen does.
    pid = re.search(r"STARTED PID=(\d+) NAME=listen", vm.command("run listen 10 &"))[1]
    vm.send("hear 1\n")
    require(vm.expect("MIND> ", timeout=60, after="hear 1\n"), "HEAR: THE MICROPHONE IS BUSY (ANOTHER PROGRAM RECORDS)")
    vm.command(f"kill {pid}")
    voice_control(vm)
    assert "FAULT PID=" not in vm.command("faults")
    print(f"PASS: microphone capture through audio_gw (48 kHz, AC97 PCM in, one owner), playback{' on an interrupt line shared with the network card, both drivers interrupted' if shared else ''}, program arguments, run by name, "
          f"speech detection on the microphone and in a WAV file ({len(starts)} phrases at {found} ms), "
          "voice commands recognized by hear, voice control in the shell (a tool started, the time spoken, a service stopped "
          "only after yes, a file read aloud, a phrase outside the grammar answered and nothing run)", flush=True)


def voice_control(vm):
    """Voice control (issue 079): the shell starts voice with a WAV file standing in for the microphone; each
    push-to-talk takes the file's next utterance, and the shell's questions take the one after."""
    output = vm.command("voice on --wav voice.wav")
    require(output, "VOICE CONTROL ON. F12 OR VOICE LISTEN")
    pid = re.search(r"STARTED PID=(\d+) NAME=voice BACKGROUND", output)[1]
    log = ""
    for _ in range(240):
        log += vm.command(f"logs {pid}")
        if "[VOICE] READY" in log:
            break
        time.sleep(.25)
    require(log, f"UTTERANCES FROM voice.wav")
    assert f"{len(DIALOGUE)} UTTERANCES FROM voice.wav" in log, log
    require(vm.command("voice on"), "ERROR: VOICE CONTROL IS ALREADY ON")
    require(vm.command("voice"), f"VOICE CONTROL ON PID={pid}")
    # Least authority: the line to the shell (SLOT_INIT), the read-only file, audio and tts clients; no clock, no loader.
    caps = vm.command(f"caps {pid}")
    endpoints = sorted(int(slot) for slot in re.findall(r"^SLOT=(\d+) GEN=\d+ endpoint ", caps, re.M))
    assert endpoints == [1, 3, 4, 6], caps
    assert not re.search(r"^SLOT=\d+ GEN=\d+ (?!endpoint|memory)", caps, re.M), caps

    def turn(until, key=False):
        # Push-to-talk (F12, or the command) and what the shell made of the utterance.
        vm.send("\x1b[24~" if key else "voice listen\n", raw=key)
        return vm.expect(until, timeout=180)
    # "открой файлы" starts fm in the foreground, with the shell's spoken answer.
    output = turn("NAME=fm FOREGROUND")
    require(output, 'VOICE: "открой файлы" (')
    require(output, ") -> Запускаю файловый менеджер")
    require(output, "VOICE: RUN fm")
    fm = re.search(r"STARTED PID=(\d+) NAME=fm FOREGROUND", output)[1]
    vm.background(fm)
    vm.command(f"kill {fm}")
    # "который час": the time in words (the synthesizer would read digits one by one).
    output = turn(" минут", key=True)
    require(output, "VOICE: LISTENING")
    assert re.search(r'VOICE: "который час" \(\d+\) -> Сейчас [а-я ]+ (час|часа|часов) [а-я ]+ минут', output), output
    # A service is stopped only after yes: "нет" cancels, "да" stops it.
    output = turn("-> Отменено")
    require(output, 'VOICE: "останови службу rtc" (')
    require(output, "-> Остановить службу rtc?")
    require(output, 'VOICE: "нет" (')
    assert "VOICE: STOPPED" not in output and "rtc" in vm.services(), output
    output = turn("VOICE: STOPPED rtc")
    require(output, 'VOICE: "да" (')
    require(output, "-> Останавливаю службу rtc")
    assert "rtc" not in vm.services()
    require(vm.command("date"), "RTC NOT AVAILABLE")
    require(vm.command("run rtc &"), "NAME=rtc")
    # A file read aloud: its first lines.
    output = turn("VOICE: READ docs/notes.txt")
    require(output, 'VOICE: "прочитай заметки" (')
    # A phrase outside the grammar is answered and runs nothing; then the file has nothing more to say.
    output = turn("-> Не понял")
    require(output, "VOICE: NOT UNDERSTOOD (CLOSEST")
    assert "VOICE: RUN" not in output and "STARTED PID=" not in output, output
    require(turn("VOICE: NOTHING HEARD"), "VOICE: LISTENING")
    time.sleep(1)
    log = vm.command(f"logs {pid}")
    for line in ("[VOICE] SAY Запускаю файловый менеджер", "[VOICE] SAY Сейчас", "[VOICE] SAY Остановить службу rtc?", "[VOICE] SAY Отменено",
                 "[VOICE] SAY Останавливаю службу rtc", "[VOICE] SAY Строка 1: съешь же ещё этих мягких французских булок, да выпей чаю. Line 1.\n",
                 "[VOICE] SAY Не понял", '[VOICE] HEARD "да" INTENT=yes', "[VOICE] NOTHING HEARD"):
        require(log, line)
    # Push-to-talk over a focused program (issue 154): F12 reaches the shell and not the program; other keys the program.
    vm.send("run keys\n")
    vm.expect("[KEYS] READY")
    start = len(vm.log)
    vm.send_bytes(b"\x1b[24~")
    vm.expect("VOICE: NOTHING HEARD", timeout=180)
    vm.send_bytes(b"q")
    vm.expect("char=q U+0071")
    assert "code=F(12)" not in vm.log[start:], vm.log[start:]
    vm.send_bytes(b"\x1b")
    vm.expect("EXITED. SHELL RESUMED.")
    # voice waits for the next push-to-talk: `voice off` answers its call with quit.
    require(vm.command("voice off"), "VOICE CONTROL OFF")
    for _ in range(40):
        if not re.search(r"^\d+ voice ", vm.command("ps", raw=True), re.M):
            break
        time.sleep(.25)
    else:
        raise AssertionError("voice did not quit")
    require(vm.command("voice"), "VOICE CONTROL OFF")
    require(vm.command("voice listen"), "ERROR: VOICE CONTROL IS OFF (VOICE ON)")
    require(vm.command("voice x"), "USAGE: VOICE ON [--wav FILE] [SECONDS] | VOICE OFF | VOICE LISTEN")


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


class _Http(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        body = f"hello from the host: {self.path}\n".encode()
        self.send_response(200); self.send_header("Content-Length", str(len(body))); self.end_headers(); self.wfile.write(body)

    def log_message(self, *args):
        pass


def _dns_server():
    # A DNS responder on the host's loopback (QEMU user networking reaches it as 10.0.2.2): names under `mind.test`
    # resolve to 10.0.2.2, everything else does not exist.
    server = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    server.bind(("127.0.0.1", 0))

    def serve():
        while True:
            try:
                data, peer = server.recvfrom(512)
            except OSError:
                return
            end = 12
            while end < len(data) and data[end] != 0:
                end += data[end] + 1
            end += 5
            found = data[12:end - 5].endswith(b"\x04mind\x04test")
            answer = data[:2] + bytes([0x81, 0x80 if found else 0x83]) + data[4:6] + (b"\x00\x01" if found else b"\x00\x00") + b"\x00" * 4 + data[12:end]
            if found:
                answer += b"\xc0\x0c\x00\x01\x00\x01\x00\x00\x00\x3c\x00\x04" + bytes([10, 0, 2, 2])
            server.sendto(answer, peer)
    threading.Thread(target=serve, daemon=True).start()
    return server


def _msix_only(vm):
    # The driver's interrupts arrive on an MSI-X vector (lines 16 and up), and it holds no legacy line.
    driver = vm.services()["virtio_net"]
    rows = [(int(line), int(count)) for line, count, holder in re.findall(r"IRQ=(\d+) COUNT=(\d+) HOLDER=(\d+)", vm.command("irqs", raw=True)) if int(holder) == driver]
    assert rows and all(line >= 16 for line, _ in rows) and any(count > 0 for _, count in rows), rows


def net_suite(args, disk):
    # Network card driver and stack in ring 3: DHCP, ICMP echo, DNS, TCP (HTTP) through QEMU's user-mode network,
    # raw frames from the driver, restart of the driver after device quiesce and of the stack.
    web = socketserver.ThreadingTCPServer(("127.0.0.1", 0), _Http)
    web.daemon_threads = True
    threading.Thread(target=web.serve_forever, daemon=True).start()
    dns = _dns_server()
    web_port, dns_port = web.server_address[1], dns.getsockname()[1]
    # The policy broker's file: netcheck may reach the host's web server and ping the gateway; rogue (a copy) nothing.
    (disk / "netpolicy.txt").write_text(f"# test policy\nnetcheck 10.0.2.2 tcp {web_port} 600 100000\nnetcheck 10.0.2.2 icmp\n")
    shutil.copyfile(disk / "netcheck.elf", disk / "rogue.elf")
    vm = VM(args, disk.relative_to(ROOT).as_posix(), extra=["-nic", "none", "-netdev", "user,id=n0", "-device", "virtio-net-pci,netdev=n0"])
    try:
        log = vm.service_logs("virtio_net", "[VIRTIO_NET] MAC=")
        require(log, "[VIRTIO_NET] MAC=52:54:00:12:34:56 LINK=UP QUEUES=256/256 MODERN MSI-X")
        # The boot report of legacy hardware: the transitional card needs no legacy code; the PIIX IDE controller does.
        report = vm.service_logs("init", "LEGACY DEVICES FOUND")
        for line in ("[INIT] LEGACY VIRTIO DEVICE WITH ONLY THE LEGACY INTERFACE: NOT FOUND", "[INIT] VIRTIO TRANSITIONAL DEVICES: 1",
                     "[INIT] LEGACY IDE CONTROLLER: FOUND 1", "[INIT] LEGACY AC97 AUDIO: NOT FOUND", "[INIT] LEGACY DEVICES FOUND: 1"):
            require(report, line)
        require(vm.command("net"), "NET MAC=52:54:00:12:34:56 LINK=UP MTU=1500")
        require(vm.service_logs("netstack", "[NETSTACK] DHCP"), "[NETSTACK] DHCP 10.0.2.15/24 GATEWAY 10.0.2.2 DNS 10.0.2.3")
        require(vm.command("ip"), "IP 10.0.2.15/24 GATEWAY 10.0.2.2 DNS 10.0.2.3 (DHCP)")
        require(vm.command("ping 10.0.2.2"), "PING: 3 SENT, 3 RECEIVED")
        require(vm.command(f"nslookup www.mind.test 10.0.2.2:{dns_port}"), "NAME www.mind.test ADDRESS 10.0.2.2")
        require(vm.command(f"nslookup missing.example 10.0.2.2:{dns_port}"), "missing.example: NotFound")
        page = vm.command(f"fetch 10.0.2.2:{web_port} /mind")
        require(page, "HTTP/1.0 200 OK"); require(page, "hello from the host: /mind")
        require(vm.command("fetch 10.0.2.2:1 /"), "FETCH: Refused")
        # Flow grants (issue 102): what the policy names and nothing else; a program without policy gets no grant.
        checks = vm.command(f"netcheck tcp:10.0.2.2:{web_port} tcp:10.0.2.2:{web_port + 1} udp:10.0.2.2:9 ping:10.0.2.2 ping:10.0.2.3")
        for line in (f"NETCHECK tcp:10.0.2.2:{web_port} OK", f"NETCHECK tcp:10.0.2.2:{web_port + 1} Denied", "NETCHECK udp:10.0.2.2:9 Denied",
                     "NETCHECK ping:10.0.2.2 OK", "NETCHECK ping:10.0.2.3 Denied"):
            require(checks, line)
        rogue = vm.command(f"rogue tcp:10.0.2.2:{web_port}")
        require(rogue, "NETWORK FOR rogue: NoPolicy"); require(rogue, f"NETCHECK tcp:10.0.2.2:{web_port} NO GRANT")
        for _ in range(20):
            if "NO NETWORK GRANTS" in vm.command("netgrants"):
                break
            time.sleep(.25)
        else:
            raise AssertionError("the grant of an ended program was not dropped")
        log = vm.command("dmesg -s netpolicy")
        for line in ("[NETPOLICY] GRANT 1 TO netcheck: 2 RULES, 600 S, 100000 BYTES", "[NETPOLICY] REFUSED rogue: NO POLICY", "[NETPOLICY] PROCESS ENDED, DROPPED 1 OF netcheck"):
            require(log, line)
        require(vm.command("dmesg -s netstack"), "[NETSTACK] DENIED FOR GRANT 1")
        # Revoking a grant removes the program's capability and closes its connection.
        holder = int(re.search(r"PID=(\d+) NAME=netcheck", vm.command(f"run netcheck hold:10.0.2.2:{web_port}:30 &"))[1])
        for _ in range(20):
            if "NETCHECK HOLDING" in vm.command(f"logs {holder}"):
                break
            time.sleep(.25)
        require(vm.command("netgrants"), "netcheck RULES=2")
        require(vm.command("netrevoke netcheck"), "REVOKED 1 GRANTS OF netcheck")
        for _ in range(20):
            ended = re.search(fr"NETCHECK hold:10.0.2.2:{web_port}:30 (NO GRANT|Denied|NoSocket|Closed)", vm.command(f"logs {holder}"))
            if ended:
                break
            time.sleep(.25)
        assert ended, vm.command(f"logs {holder}")
        require(vm.command("dmesg -s netpolicy"), "[NETPOLICY] REVOKED 2 OF netcheck: 1 COPIES REMOVED, 1 SOCKETS CLOSED")
        counters = re.search(r"SENT=(\d+) RECEIVED=(\d+) DROPPED=(\d+) INTERRUPTS=(\d+)", vm.command("net"))
        assert counters and int(counters[1]) >= 5 and int(counters[2]) >= 5 and int(counters[4]) >= 1, counters  # sent, received, interrupts
        _msix_only(vm)
        # A killed stack is restarted by init and configures itself again.
        require(vm.command(f"kill {vm.services()['netstack']}", raw=True), "KILLED PID=")
        require(vm.service_logs("init", "netstack RESTARTED"), "netstack RESTARTED")
        for _ in range(40):
            if "3 RECEIVED" in vm.command("ping 10.0.2.2"):
                break
            time.sleep(.25)
        else:
            raise AssertionError("no ping answer after the stack restart")
        # Raw frames: with the stack stopped, the shell's diagnostics get the ARP answer themselves.
        require(vm.command("svc stop netstack"), "netstack stopped")
        require(vm.command("net arp 10.0.2.2"), "ARP 10.0.2.2 IS AT 52:55:0A:00:02:02")
        require(vm.command("net arp 10.0.2.99"), "ARP 10.0.2.99: NO ANSWER")
        require(vm.command("ping 10.0.2.2"), "NET: NO NETWORK STACK")
        require(vm.command("svc start netstack"), "netstack started: PID")
        # init stops the device and clears its DMA region before the driver starts again; the stack carries on.
        require(vm.command(f"kill {vm.services()['virtio_net']}", raw=True), "KILLED PID=")
        log = vm.service_logs("init", "virtio_net RESTARTED")
        assert log.index("virtio_net DEVICE QUIESCED") < log.index("virtio_net RESTARTED"), log
        for _ in range(40):
            if "3 RECEIVED" in vm.command("ping 10.0.2.2"):
                break
            time.sleep(.25)
        else:
            raise AssertionError("no ping answer after the driver restart")
        # The new driver instance had no frame ring: the stack lent it a fresh one (issue 107).
        require(vm.command("dmesg -s netstack"), "[NETSTACK] CARD 0: DRIVER WITHOUT OUR RING, ATTACHING AGAIN")
    finally:
        vm.close()
        web.shutdown(); dns.close()
        (Path(tempfile.gettempdir()) / f"mind-core-net-{args.cpus}cpu.log").write_text(vm.log)
    # Two cards on two user-mode networks (issue 105): a driver instance and an interface each, flows routed by network,
    # and one driver's restart leaves the other card working and gives the new instance its own card back.
    web = socketserver.ThreadingTCPServer(("127.0.0.1", 0), _Http)
    web.daemon_threads = True
    threading.Thread(target=web.serve_forever, daemon=True).start()
    web_port = web.server_address[1]
    two = ["-nic", "none", "-netdev", "user,id=n0", "-device", "virtio-net-pci,netdev=n0,mac=52:54:00:12:34:56",
           "-netdev", "user,id=n1,net=10.0.3.0/24", "-device", "virtio-net-pci,netdev=n1,mac=52:54:00:12:34:57"]
    vm = VM(args, disk.relative_to(ROOT).as_posix(), extra=two)
    try:
        require(vm.service_logs("virtio_net", "[VIRTIO_NET] MAC="), "[VIRTIO_NET] MAC=52:54:00:12:34:56")
        require(vm.service_logs("virtio_net#1", "[VIRTIO_NET] MAC="), "[VIRTIO_NET] MAC=52:54:00:12:34:57")
        log = vm.service_logs("netstack", "ON CARD 1")
        require(log, "[NETSTACK] DHCP 10.0.2.15/24 GATEWAY 10.0.2.2 DNS 10.0.2.3 ON CARD 0")
        require(log, "[NETSTACK] DHCP 10.0.3.15/24 GATEWAY 10.0.3.2 DNS 10.0.3.3 ON CARD 1")

        def frames():
            found = dict((int(card), (int(sent), int(received))) for card, sent, received in re.findall(r"CARD (\d) MAC .* SENT=(\d+) RECEIVED=(\d+)", vm.command("ip")))
            assert set(found) == {0, 1}, found
            return found
        before = frames()
        require(vm.command("ping 10.0.3.2"), "PING: 3 SENT, 3 RECEIVED")
        require(vm.command(f"fetch 10.0.3.2:{web_port} /second"), "hello from the host: /second")
        after = frames()
        assert after[1][0] - before[1][0] >= 6 and after[0][0] - before[0][0] < 3, (before, after)  # the second network's card
        require(vm.command(f"fetch 10.0.2.2:{web_port} /first"), "hello from the host: /first")
        assert frames()[0][0] - after[0][0] >= 3, (after, frames())
        # The second driver dies: the first card keeps working, the restarted instance drives the second card again.
        require(vm.command(f"kill {vm.services()['virtio_net#1']}", raw=True), "KILLED PID=")
        require(vm.command("ping 10.0.2.2"), "PING: 3 SENT, 3 RECEIVED")
        log = vm.service_logs("init", "virtio_net#1 RESTARTED")
        assert log.index("virtio_net#1 DEVICE QUIESCED") < log.index("virtio_net#1 RESTARTED"), log
        require(vm.service_logs("virtio_net#1", "[VIRTIO_NET] MAC="), "[VIRTIO_NET] MAC=52:54:00:12:34:57")
        for _ in range(40):
            if "3 RECEIVED" in vm.command("ping 10.0.3.2"):
                break
            time.sleep(.25)
        else:
            raise AssertionError("no ping answer on the second card after its driver's restart")
        require(vm.command("svc"), "virtio_net#1")
    finally:
        vm.close()
        web.shutdown()
    # A modern-only card (no legacy registers) and a legacy-only one (no modern structures, no MSI-X).
    for device, mode in [("virtio-net-pci,netdev=n0,disable-legacy=on", "MODERN MSI-X"), ("virtio-net-pci,netdev=n0,disable-modern=on", "LEGACY INTX")]:
        vm = VM(args, disk.relative_to(ROOT).as_posix(), extra=["-nic", "none", "-netdev", "user,id=n0", "-device", device])
        try:
            require(vm.service_logs("virtio_net", "[VIRTIO_NET] MAC="), mode)
            report = vm.service_logs("init", "LEGACY DEVICES FOUND")
            require(report, "[INIT] LEGACY VIRTIO DEVICE WITH ONLY THE LEGACY INTERFACE: " + ("FOUND 1" if mode == "LEGACY INTX" else "NOT FOUND"))
            assert ("VIRTIO TRANSITIONAL DEVICES" in report) is False, report
            require(vm.service_logs("netstack", "[NETSTACK] DHCP"), "[NETSTACK] DHCP 10.0.2.15/24")
            require(vm.command("ping 10.0.2.2"), "PING: 3 SENT, 3 RECEIVED")
            if mode == "MODERN MSI-X":
                _msix_only(vm)
        finally:
            vm.close()
    # QEMU's default e1000 has the same PCI class: it is not taken for a VirtIO card; the stack reports no network.
    vm = VM(args, disk.relative_to(ROOT).as_posix())
    try:
        assert "virtio_net" not in vm.services()
        require(vm.service_logs("init", "virtio_net NOT STARTED"), "virtio_net NOT STARTED: NO DEVICE")
        require(vm.command("net"), "NET: NO NETWORK CARD")
        require(vm.command("ip"), "IP: NoNetwork")
    finally:
        vm.close()
    print("PASS: VirtIO network card and network stack in ring 3: DHCP, ping, DNS, TCP/HTTP, refused connection, "
          "flow grants of the policy broker (allowed, denied, no policy, dropped at exit, revoked), two cards on two networks "
          "(a driver instance and an interface each, routes by network, one driver restarted with its own card), "
          "raw ARP through the driver, restarts of the stack and of the driver after device quiesce; modern interface with MSI-X "
          "(transitional and modern-only cards), legacy interface; e1000 not taken", flush=True)


def _certificates(directory):
    # A test CA (RSA) and a server certificate it signs (ECDSA P-256) for mind.test and 10.0.2.2; a second CA nobody
    # trusts and a server certificate from it for the same names.
    def run(*command):
        subprocess.run(["openssl", *command], cwd=directory, check=True, capture_output=True)
    names = directory / "names.cnf"
    names.write_text("subjectAltName=DNS:mind.test,IP:10.0.2.2\nextendedKeyUsage=serverAuth\nbasicConstraints=critical,CA:FALSE\n")
    for ca in ("ca", "rogue-ca"):
        run("req", "-x509", "-newkey", "rsa:2048", "-nodes", "-keyout", f"{ca}.key", "-out", f"{ca}.pem", "-days", "2",
            "-subj", f"/CN=MIND {ca}", "-addext", "basicConstraints=critical,CA:TRUE", "-addext", "keyUsage=critical,keyCertSign")
        server = "server" if ca == "ca" else "rogue"
        run("req", "-newkey", "ec", "-pkeyopt", "ec_paramgen_curve:P-256", "-nodes", "-keyout", f"{server}.key", "-out", f"{server}.csr", "-subj", "/CN=mind.test")
        run("x509", "-req", "-in", f"{server}.csr", "-CA", f"{ca}.pem", "-CAkey", f"{ca}.key", "-CAcreateserial", "-out", f"{server}.pem", "-days", "2", "-extfile", "names.cnf")


class _Https(_Http):
    def do_GET(self):
        peer = self.request.getpeercert()
        client = dict(item[0] for item in peer["subject"])["commonName"] if peer else "nobody"
        body = f"hello from the host: {self.path} to {client}\n".encode()
        self.send_response(200); self.send_header("Content-Length", str(len(body))); self.end_headers(); self.wfile.write(body)


def _https_server(directory, certificate):
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    context.load_cert_chain(directory / f"{certificate}.pem", directory / f"{certificate}.key")
    server = socketserver.ThreadingTCPServer(("127.0.0.1", 0), _Https)
    server.daemon_threads = True
    server.socket = context.wrap_socket(server.socket, server_side=True)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server, context


def tls_suite(args, disk):
    # TLS service and key service (issue 103): HTTPS with the server certificate verified against tlsroots.pem, wrong
    # name and untrusted CA refused, the device certificate (key in the key service only) for a server that asks for
    # one; without RDRAND both services fail closed.
    certificates = Path(tempfile.mkdtemp(prefix="mind-tls-"))
    _certificates(certificates)
    shutil.copyfile(certificates / "ca.pem", disk / "tlsroots.pem")
    good, _ = _https_server(certificates, "server")
    rogue, _ = _https_server(certificates, "rogue")
    asking, asking_context = _https_server(certificates, "server")
    ports = {name: server.server_address[1] for name, server in (("good", good), ("rogue", rogue), ("asking", asking))}
    network = ["-nic", "none", "-netdev", "user,id=n0", "-device", "virtio-net-pci,netdev=n0"]
    vm = VM(args, disk.relative_to(ROOT).as_posix(), rtc="utc", extra=[*network, "-cpu", "qemu64,+rdrand"])
    try:
        require(vm.service_logs("keystore", "DEVICE KEY READY"), "[KEYSTORE] DEVICE KEY READY: MIND ")
        require(vm.service_logs("tls", "[TLS] READY"), "[TLS] READY: TLS 1.3 CLIENT, ROOTS FROM tlsroots.pem, RANDOM FROM RDRAND")
        require(vm.service_logs("netstack", "[NETSTACK] DHCP"), "[NETSTACK] DHCP 10.0.2.15/24")
        page = vm.command(f"https 10.0.2.2:{ports['good']} /secure mind.test")
        for line in ("HTTPS: mind.test VERIFIED, TLS 1.3 SUITE", "HTTP/1.0 200 OK", "hello from the host: /secure to nobody"):
            require(page, line)
        assert "CLIENT CERTIFICATE SENT" not in page, page
        require(vm.command(f"https 10.0.2.2:{ports['good']} /by-address"), "hello from the host: /by-address to nobody")
        # The other cipher suites and the other key exchange group, from an OpenSSL server limited to each.
        for suite, number, group, group_number in (("TLS_CHACHA20_POLY1305_SHA256", "1303", "X25519", "001D"), ("TLS_AES_128_GCM_SHA256", "1301", "P-256", "0017")):
            with socket.socket() as probe:
                probe.bind(("127.0.0.1", 0)); port = probe.getsockname()[1]
            server = subprocess.Popen(["openssl", "s_server", "-accept", f"127.0.0.1:{port}", "-cert", "server.pem", "-key", "server.key", "-www",
                                       "-tls1_3", "-ciphersuites", suite, "-groups", group], cwd=certificates, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            try:
                for _ in range(40):
                    with socket.socket() as probe:
                        if probe.connect_ex(("127.0.0.1", port)) == 0:
                            break
                    time.sleep(.1)
                page = vm.command(f"https 10.0.2.2:{port} / mind.test")
                require(page, f"VERIFIED, TLS 1.3 SUITE {number}"); require(page, "HTTP/1.0 200 ok")
                require(vm.command("dmesg -s tls"), f"SUITE {number}, GROUP {group_number}")
            finally:
                server.kill(); server.wait()
        require(vm.command(f"https 10.0.2.2:{ports['good']} / other.test"), "HTTPS: Certificate")
        require(vm.command(f"https 10.0.2.2:{ports['rogue']} / mind.test"), "HTTPS: Certificate")
        require(vm.command("https 10.0.2.2:1 / mind.test"), "HTTPS: Refused")
        log = vm.command("dmesg -s tls")
        for line in ("mind.test VERIFIED, SUITE", "other.test REFUSED: CERTIFICATE NotValidForName", "mind.test REFUSED: CERTIFICATE UnknownIssuer"):
            require(log, line)
        # The device certificate: the server trusts exactly it; offered only with -c.
        pem = re.search(r"-----BEGIN CERTIFICATE-----\r?\n.*?-----END CERTIFICATE-----", vm.command("tls cert"), re.S)[0].replace("\r", "")
        der = ssl.PEM_cert_to_DER_cert(pem)
        name = re.search(r"MIND [0-9A-F]{8}", der.decode("latin-1"))[0]
        asking_context.verify_mode = ssl.CERT_REQUIRED
        asking_context.load_verify_locations(cadata=pem)
        # TLS 1.3: the server refuses after the client's Finished; its alert may be lost to the RST of the closed socket.
        page = vm.command(f"https 10.0.2.2:{ports['asking']} /refused mind.test")
        assert "200 OK" not in page and ("HTTPS: Handshake" in page or "HTTPS: 0 BYTES" in page), page
        page = vm.command(f"https -c 10.0.2.2:{ports['asking']} /device mind.test")
        require(page, "CLIENT CERTIFICATE SENT"); require(page, f"hello from the host: /device to {name}")
        require(vm.command("dmesg -s keystore"), "[KEYSTORE] SIGNED TlsClient FOR PID")
        # Besides init (which keeps every service endpoint and minted client for restarts) and the key service itself,
        # only the TLS service holds a capability to the key service, with the signer's badge.
        servers = {int(ep): int(server) for ep, server in re.findall(r"^EP=(\d+) .*SERVER=(\d+)", vm.command("endpoints", raw=True), re.M)}
        real = vm.services()
        keystore = [ep for ep, server in servers.items() if server == real["keystore"]]
        holders = [service for service, pid in real.items() if service not in ("init", "keystore") and any(re.search(fr"BADGE=\d+ EP={ep}\b", vm.command(f"stat caps {pid}", raw=True)) for ep in keystore)]
        assert holders == ["tls"], holders
        assert re.search(fr"BADGE=1 EP={keystore[0]}\b", vm.command(f"stat caps {real['tls']}", raw=True))
    finally:
        vm.close()
        for server in (good, rogue, asking):
            server.shutdown()
        (Path(tempfile.gettempdir()) / f"mind-core-tls-{args.cpus}cpu.log").write_text(vm.log)
    vm = VM(args, disk.relative_to(ROOT).as_posix(), rtc="utc", extra=network)
    try:
        require(vm.service_logs("keystore", "NO RDRAND"), "[KEYSTORE] NO RDRAND: NO DEVICE KEY")
        require(vm.service_logs("tls", "[TLS] READY"), "NO RDRAND: EVERY CONNECTION WILL BE REFUSED")
        require(vm.command(f"https 10.0.2.2:{ports['good']} / mind.test"), "HTTPS: NoEntropy")
        require(vm.command("tls cert"), "TLS: NotFound")
    finally:
        vm.close()
    shutil.rmtree(certificates, ignore_errors=True)
    print("PASS: TLS 1.3 client service: HTTPS with the server certificate verified (by name and by address; AES-256-GCM, "
          "AES-128-GCM and ChaCha20-Poly1305; X25519 and P-256), wrong name, "
          "untrusted CA and refused port reported; the device certificate offered with -c and signed for by the key service, "
          "which only the TLS service may ask; no RDRAND: no key and no connection", flush=True)


class _Bench(socketserver.StreamRequestHandler):
    # netbench's TCP side: `DOWN n` sends n bytes and closes; `UP n` reads n bytes and answers `OK n`.
    def handle(self):
        command, count = self.rfile.readline().split()
        count = int(count)
        if command == b"DOWN":
            block = bytes(range(256)) * 256
            while count > 0:
                self.wfile.write(block[:min(count, len(block))]); count -= len(block)
        else:
            while count > 0:
                data = self.rfile.read(min(count, 65536))
                if not data:
                    return
                count -= len(data)
            self.wfile.write(f"OK {count}\n".encode())


def _bench_server(host):
    # netbench's host side: TCP on a free port and a UDP echo on the same port number.
    tcp = socketserver.ThreadingTCPServer((host, 0), _Bench)
    tcp.daemon_threads = True
    threading.Thread(target=tcp.serve_forever, daemon=True).start()
    udp = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    udp.bind((host, tcp.server_address[1]))

    def echo():
        while True:
            try:
                data, peer = udp.recvfrom(2048)
                udp.sendto(data, peer)
            except OSError:
                return
    threading.Thread(target=echo, daemon=True).start()
    return tcp, udp


def netbench_suite(args, disk):
    # Network stack benchmark (issue 106): TCP down/upload and UDP round trips through virtio_net and netstack, with
    # the CPU time of the stack and the drivers. The numbers are printed (and recorded in docs/profile/network.md); the
    # suite checks only that every phase completes. QEMU's user networking has no virtio-net header, so its card offers
    # no offloads; with --tap (a tap interface at 10.0.2.2/24 on the host, see .github/workflows/ci.yml) the card offers
    # checksum offload and both settings are measured (the host's TCP drops segments with a wrong checksum).
    tcp, udp = _bench_server("10.0.2.2" if args.tap else "127.0.0.1")
    port = tcp.server_address[1]
    (disk / "netpolicy.txt").write_text(f"netbench 10.0.2.2 tcp {port} 3600 1000000000\nnetbench 10.0.2.2 udp {port}\n")
    backend = f"tap,id=n0,ifname={args.tap},script=no,downscript=no,vnet_hdr=on" if args.tap else "user,id=n0"
    vm = VM(args, disk.relative_to(ROOT).as_posix(), extra=["-nic", "none", "-netdev", backend, "-device", "virtio-net-pci,netdev=n0"])
    try:
        require(vm.service_logs("netstack", "10.0.2.15/24"), "STATIC 10.0.2.15/24" if args.tap else "DHCP 10.0.2.15/24")
        log = vm.command("dmesg -s netstack")
        require(log, "[NETSTACK] CARD 0 READY (FRAME RING)")
        offers = "(CHECKSUM OFFLOAD AVAILABLE)" in log
        assert offers == bool(args.tap), "checksum offload offered only with the tap backend"
        for offload in ("off", "on"):
            require(vm.command(f"ip offload {offload}"), f"CHECKSUM OFFLOAD {offload.upper()}: {int(offers and offload == 'on')} CARD(S)")
            for run in range(args.bench_runs):
                vm.send(f"netbench 10.0.2.2:{port} {args.bench_mib}\n")
                output = vm.expect("MIND> ", timeout=600)
                for phase in ("DOWN", "UP"):
                    assert re.search(fr"NETBENCH {phase} {args.bench_mib * 1048576} BYTES \d+ MS [1-9]\d* KIB/S", output), output
                assert re.search(r"NETBENCH UDP 500 OF 500 ROUND TRIPS", output), output
                for line in re.findall(r"NETBENCH .*", output):
                    print(f"  offload {offload} run {run + 1}: " + line.strip(), flush=True)
    finally:
        vm.close()
        tcp.shutdown(); udp.close()
    print(f"PASS: netbench ({'tap, checksum offload offered' if args.tap else 'user networking, no offloads offered'}): TCP download and "
          "upload, UDP round trips, CPU time of netstack and the drivers, offload off and on", flush=True)


def wm_suite(vm):
    """The window manager (issue 088): fm, clock and top in three windows on one screen (text frames and content, the
    clock's pixels); keys reach only the window in front; halves, quarters, maximize and snapping by keys and by
    dragging a title with the mouse; a program started from wm gets only what wm holds; leaving wm and killing it keep
    the programs running and the next wm shows them where they were; close all ends them."""
    # The broker's first window gives it a heap arena it keeps: one window before the baseline.
    require(vm.command("wintest show W 1"), "W DONE AFTER")
    time.sleep(1.2)  # the broker frees a window within 500 ms of its program's end
    baseline = heap_used(vm)
    windows_re = re.compile(r'\[WM\] WINDOW (\d+) PID (\d+) (TEXT|PIXELS) (\d+)X(\d+) "[^"]*" AT (\d+),(\d+) (\d+)X(\d+)')
    seen = []  # everything wm logged, PIDs as the suites number them
    read = [len(vm.log)]  # how far into the log: lines printed while in the QEMU monitor count too

    def wait(text=None, lines=1, timeout=12):
        # Until `text` and `lines` state lines (one per key) have come; returns what came.
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            vm.collect()
            fresh = vm.log[read[0]:]
            done = to_ordinal(ANSI.sub("", fresh[:fresh.rfind("\n") + 1]).replace("\r", ""))
            if done.count("[WM] MODE=") >= lines and (text is None or text in done):
                read[0] += fresh.rfind("\n") + 1  # a line still coming stays for the next wait
                seen.append(done)
                return done
            time.sleep(.02)
        raise AssertionError(f"Timeout waiting for {text!r}: {vm.log[read[0]:][-3000:]}")

    def until(text):
        # `text` was logged already, or comes.
        if text not in "".join(seen):
            wait(text, lines=0)

    def state():
        # The windows bottom to top as {id: (x, y, w, h)}, the focus and the mode, from the last state line.
        mode, focus, windows = re.findall(r"\[WM\] MODE=(\w+) FOCUS=(\S+) WINDOWS=(.*?)(?: POINTER=\S+)?$", "".join(seen), re.M)[-1]
        rects = {int(i): tuple(map(int, (x, y, w, h))) for i, x, y, w, h in re.findall(r"(\d+)@(\d+),(\d+),(\d+)x(\d+)", windows)}
        return mode, (int(focus) if focus != "-" else None), rects

    def keys(*names, text=None):
        # PS/2 keys from the QEMU monitor (Alt combinations cannot come over the UART); back without an Enter.
        for name in names:
            vm.hmp(f"sendkey {name}")
            time.sleep(.08)
        vm.serial(enter=False)
        wait(text, lines=len(names))
        return state()

    def front(window):
        # Alt+Tab until `window` is in front.
        return keys(*["alt-tab"] * (list(state()[2]).index(window) + 1))

    vm.send("wm fm, clock, top\n")
    out = wait()
    while len(windows_re.findall("".join(seen))) < 3:
        out = wait("[WM] WINDOW", lines=0)
    out = "".join(seen)
    started = dict(re.findall(r"\[WM\] STARTED (\w+) PID (\d+) WITH", out))
    wm_pid = int(re.search(r"STARTED PID=(\d+) NAME=wm FOREGROUND", out)[1])
    windows = {int(m[0]): m for m in windows_re.findall(out)}
    by_name = {name: next(i for i, m in windows.items() if m[1] == pid) for name, pid in started.items()}
    fm, clock, top = by_name["fm"], by_name["clock"], by_name["top"]
    # Programs get what wm holds and they ask for: fm the user's files, top system information; not top's
    # lifecycle client, which wm does not hold.
    require(out, f"STARTED fm PID {started['fm']} WITH window,files")
    require(out, f"STARTED top PID {started['top']} WITH window,sysinfo WITHOUT lifecycle")
    assert windows[clock][2:5] == ("PIXELS", "320", "176") and windows[fm][2] == "TEXT", windows
    time.sleep(1.5)  # the text programs drew at the size of their frames
    mode, focus, rects = front(clock)  # the clock in front: no key of the harness reaches a program that uses it
    assert focus == clock and mode == "NORMAL", (focus, rects)
    # The screen: the top bar, text frames with their titles and content, the clock's pixels in its frame.
    screen = screen_text(vm)
    _, size, _, pixels = vm.screenshot().split(b"\n", 3)
    vm.serial(enter=False)
    assert screen[0].startswith(canon(" wm │ Alt+Tab next")), screen[0]
    x, y, w, h = rects[fm]
    assert canon(" fm A:/ ") in screen[y][x:x + w] and canon("Name") in screen[y + 2][x:x + w], screen[y:y + 3]
    x, y, w, h = rects[top]
    assert canon(" top ") in screen[y][x:x + w] and any(canon("PID NAME") in row[x:x + w] for row in screen[y:y + h]), screen[y:y + h]
    width = int(size.split()[0])
    x, y, w, h = rects[clock]
    green = sum(pixels[(py * width + px) * 3:(py * width + px) * 3 + 3] == bytes((0xA6, 0xE3, 0xA1))
                for py in range((y + 1) * 16, (y + h - 1) * 16) for px in range((x + 1) * 8, (x + w - 1) * 8))
    assert green > 500, green  # the clock's digits, drawn by clock into its pixel window
    assert canon(" clock ") in screen[y][x:x + w], screen[y]
    # Keys reach only the window in front: fm gets "cd docs", top nothing.
    mode, focus, rects = front(fm)
    assert focus == fm, (focus, rects)
    vm.send_bytes(b"cd docs\r")
    wait(lines=8)
    vm.background(wm_pid)
    fm_log = vm.command(f"logs {started['fm']}")
    require(fm_log, "CMD=cd docs")
    require(fm_log, "LEFT=/docs FULL")
    assert "[TOP] " not in vm.command(f"logs {started['top']}").replace("[TOP] READY", ""), "top got no key"
    vm.send(f"fg {wm_pid}\n")
    vm.expect(f"FOREGROUND PID={wm_pid}")
    # Halves, quarters, maximize and back.
    assert keys("alt-right")[2][fm] == (80, 1, 80, 48)
    assert keys("alt-3")[2][fm] == (0, 25, 80, 24)
    assert keys("alt-ret")[2][fm] == (0, 1, 160, 48)
    assert keys("alt-ret")[2][fm] == (0, 25, 80, 24)
    # Alt+M: off the left edge and back; Enter snaps it to the left half.
    mode, focus, rects = keys("alt-m", *["up"] * 5, *["right"] * 10)
    assert mode == "MOVE" and rects[fm] == (10, 20, 80, 24), (mode, rects)
    mode, focus, rects = keys("ctrl-left", "ctrl-left", "ret")
    assert mode == "NORMAL" and rects[fm] == (0, 1, 80, 48), (mode, rects)
    # The mouse: the pointer starts in the middle of the screen (cell 80, 25); the clock's title is dragged to the
    # left edge, below the top, and let go: it snaps to the left half.
    x, y, w, h = rects[clock]
    dx, dy = (x + 4) * 8 + 4 - 640, y * 16 + 8 - 400
    for move in [f"mouse_move {dx // 4} 0"] * 4 + [f"mouse_move 0 {dy // 4}"] * 4 + ["mouse_button 1"] + ["mouse_move -100 0"] * 8 + ["mouse_move 0 80"] * 2 + ["mouse_button 0"]:
        vm.hmp(move)
        time.sleep(.12)
    vm.serial(enter=False)
    wait(lines=2)
    mode, focus, rects = state()
    assert focus == clock and rects[clock] == (0, 1, 80, 48), (focus, rects)
    # A program started from wm that asks for more than wm holds runs without it: caps has no authority view.
    keys("alt-r", "c", "a", "p", "s", "ret", text="STARTED caps")
    caps_pid = re.findall(r"\[WM\] STARTED caps PID (\d+) WITH window WITHOUT authority", "".join(seen))[-1]
    while not any(m[1] == caps_pid for m in windows_re.findall("".join(seen))):
        wait("[WM] WINDOW", lines=0)
    caps = next(int(m[0]) for m in windows_re.findall("".join(seen)) if m[1] == caps_pid)
    time.sleep(1.5)
    screen = screen_text(vm)
    vm.serial(enter=False)
    assert any(canon("No access to sysmon") in row for row in screen), screen
    keys("alt-w", text=f"CLOSE {caps}")
    until(f"GONE {caps}")
    while caps in state()[2]:
        wait()  # the state line after the window went
    # Leaving: the programs keep running; the next wm shows them where they were.
    places = state()[2]
    assert set(places) == {fm, clock, top}, places
    vm.hmp("sendkey alt-q"); vm.serial(enter=False)
    require(wait("EXITED. SHELL RESUMED.", lines=0), "DETACHED: 3 WINDOWS KEPT")
    time.sleep(.1); vm.collect(); vm.output = ""
    names = {row[0] for row in task_rows(vm).values()}
    assert {"fm", "clock", "top"} <= names and "wm" not in names, names

    def again():
        read[0] = len(vm.log)
        vm.send("wm\n")
        out = wait("[WM] READY", lines=0)
        restored = {int(m[0]): tuple(map(int, m[5:9])) for m in windows_re.findall(out)}
        assert restored == places, (restored, places)
        return int(re.search(r"STARTED PID=(\d+) NAME=wm FOREGROUND", out)[1])

    second = again()
    # A killed wm is detached by the broker: the programs still run, and the next wm shows them.
    time.sleep(.5)
    vm.background(second)
    require(vm.command(f"kill {second}"), "KILLED")
    time.sleep(1.5)
    assert {"fm", "clock", "top"} <= {row[0] for row in task_rows(vm).values()}
    again()
    time.sleep(.5)
    # Close all: every program ends, then wm.
    vm.hmp("sendkey alt-x"); vm.serial(enter=False)
    require(wait("EXITED. SHELL RESUMED.", lines=0), "CLOSE ALL: 3 WINDOWS")
    time.sleep(1); vm.collect(); vm.output = ""
    assert task_rows(vm) == {}, task_rows(vm)
    for _ in range(20):
        if heap_used(vm) == baseline:
            break
        time.sleep(.2)
    assert heap_used(vm) == baseline
    print("PASS: wm: fm, clock and top in windows (text frames and content, the clock's pixels); keys to the window in front only; "
          "halves, quarters, maximize, Alt+M and snapping, a title dragged with the mouse; programs get only what wm holds; "
          "leaving and a killed wm keep the programs and the next wm restores the places; close all ends them", flush=True)


def windows_suite(vm):
    # Window broker (issue 157): windows outlive their manager; a new manager gets them back where they were; one
    # manager at a time; close all ends the programs; a plain client cannot act as manager or read others' windows.
    pids = [int(re.search(r"PID=(\d+) NAME=wintest", vm.command(f"run wintest show {title} 120 &"))[1]) for title in ("ALPHA", "BETA")]
    time.sleep(1)
    first = vm.command("winmgr manage 0")
    for title in ("ALPHA", "BETA"):
        assert re.search(fr'WINDOW \d+ "{title}" Text 24X2 PLACE 0,0 ROW "{title} TICK \d+"', first), first
    require(vm.command("dmesg -s windows"), "MANAGER PID")
    time.sleep(1.2)  # the broker sees the manager has ended
    require(vm.command("dmesg -s windows"), "ENDED: 2 WINDOWS KEPT")
    for pid in pids:
        log = vm.command(f"logs {pid}")
        for line in ("STATE SHOWN", "EVENT CHAR k", "STATE HIDDEN"):
            require(log, line)
    ticks = [int(re.findall(r"TICK (\d+)", vm.command("winmgr manage 0"))[-1])]
    second = vm.command("winmgr manage 0")
    assert re.search(r'WINDOW (\d+) "ALPHA" Text 24X2 PLACE (\d+),(\d+)', second), second
    window, x, y = map(int, re.search(r'WINDOW (\d+) "ALPHA" Text 24X2 PLACE (\d+),(\d+)', second).groups())
    assert (x, y) == (2 * window, window), second  # the place the previous manager saved
    assert int(re.findall(r"TICK (\d+)", second)[-1]) > ticks[0], second  # the programs kept drawing
    # One manager at a time; a plain client is neither manager nor reader of other windows.
    holder = int(re.search(r"PID=(\d+) NAME=winmgr", vm.command("run winmgr manage 5 &"))[1])
    time.sleep(1)
    require(vm.command("winmgr second"), "SECOND MANAGER: Ok(Err(Busy))")
    intruder = vm.command("wintest intrude")
    require(intruder, "INTRUDER ATTACH: Ok(Err(Denied))")
    require(intruder, "INTRUDER LIST: Ok(Err(Denied))")
    assert "Ok(Ok(" not in intruder.split("INTRUDER SURFACE")[1], intruder
    for _ in range(40):
        if "MANAGER LEAVES" in vm.command(f"logs {holder}"):
            break
        time.sleep(.25)
    time.sleep(1.2)
    # Close all: every program is asked to end and its window goes.
    require(vm.command("winmgr closeall"), "CLOSE ALL: Ok(Ok(2))")
    seen = ""  # `logs` drains, and an ended program's buffer goes with it: keep what each call returned
    for _ in range(40):
        seen += "".join(vm.command(f"logs {pid}") for pid in pids)
        if "wintest" not in vm.command("ps"):
            break
        time.sleep(.25)
    else:
        raise AssertionError(seen)
    # A program may print its last line and end between the reads above and `ps`: its output stays readable once.
    seen += "".join(vm.command(f"logs {pid}") for pid in pids)
    require(seen, "CLOSED AFTER")
    time.sleep(1.2)
    require(vm.command("dmesg -s windows"), "ENDED WITH PID")
    print("PASS: window broker: windows outlive their manager (hidden, still drawn), a new manager gets them at their "
          "saved places, one manager at a time, plain clients refused, close all ends the programs", flush=True)


def display_suite(args, disk):
    # Colours are right on every QEMU display adapter: the compositor converts to the framebuffer's pixel format.
    for name, display in [("std", ["-vga", "std"]), ("virtio", ["-vga", "virtio"]), ("ramfb", ["-vga", "none", "-device", "ramfb"])]:
        vm = VM(args, disk.relative_to(ROOT).as_posix(), extra=display)
        try:
            require(vm.command("run app &"), "PID=1 NAME=app BACKGROUND")
            vm.send("fg 1\n")
            vm.expect("FOREGROUND PID=1")
            time.sleep(.3)
            assert center_pixel(vm) == b"\x00\xff\xff", (name, center_pixel(vm))
            vm.serial()  # the screenshot switched the console to the QEMU monitor
            vm.send(" \n")
            time.sleep(.3)
            assert center_pixel(vm) == b"\xff\x00\x00", (name, center_pixel(vm))
        finally:
            vm.close()
    print("PASS: cyan and red reach the screen unchanged on VGA std, virtio-vga and ramfb", flush=True)


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
    # REBOOT resets the machine and the firmware boots the image again: on q35 through the FADT reset register; the
    # i440fx `pc` machine has a revision 1 FADT without one, so the kernel falls back to port 0xCF9.
    for machine, method in [((), "PORT 0xCF9"), (("-machine", "q35"), "ACPI RESET REGISTER")]:
        vm = VM(args, disk.relative_to(ROOT).as_posix(), reboot=True, extra=machine)
        try:
            vm.send("reboot\n")
            vm.expect(f"MIND CORE KERNEL: REBOOT VIA {method}", timeout=30)
            vm.expect("MIND CORE KERNEL: INIT STARTED", timeout=90)
            vm.expect("MIND>", timeout=60)
        finally:
            vm.close()
    print("PASS: reboot through the ACPI reset register (q35) and port 0xCF9 (pc); the system boots again", flush=True)
    if args.panic_kernel:
        # A kernel panic reports message, location, CPU and the running task, even inside the scheduler lock.
        target = disk / "kernel.elf"
        target.write_bytes(Path(args.panic_kernel).read_bytes())
        vm = VM(args, disk.relative_to(ROOT).as_posix(), prompt=False)
        try:
            deadline, pattern = time.monotonic() + 30, re.compile(r"KERNEL PANIC: panic test at src/scheduler\.rs:\d+:\d+ CPU=\d+ PID=\d+ NAME=init\n")
            while not pattern.search(vm.output.replace("\r", "")):
                assert time.monotonic() < deadline and vm.process.poll() is None, vm.output[-2000:]
                vm.collect(); time.sleep(.05)
        finally:
            vm.close()
        target.write_bytes(kernel)
        print("PASS: kernel panic report names message, source location, CPU and running task", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--qemu", default=os.environ.get("QEMU", "qemu-system-x86_64"))
    parser.add_argument("--cpus", type=int, default=4)
    parser.add_argument("--cpu-model", help="QEMU -cpu model, e.g. max: AVX state saved with XSAVE (issue 153)")
    parser.add_argument("--firmware", default="OVMF.fd")
    parser.add_argument("--busy-elf", help="test-only ELF built from tests/busy_app.rs")
    parser.add_argument("--isolation-elf", help="test-only ELF built from tests/isolation_app.rs")
    parser.add_argument("--heap-elf", help="test-only ELF built from tests/heap_app.rs")
    parser.add_argument("--block-elf", help="test-only ELF built from tests/block_app.rs (stands in for vfs_server)")
    parser.add_argument("--panic-kernel", help="test-only kernel built with --features panic-test (boot suite)")
    parser.add_argument("--suites", help="comma-separated subset: boot,display,net,tls,netbench,windows,wm,normal,memory,dzen,services,ahci,audio,tts,listen,keys,shell,tools,vfs,edit,disk,busy,smp,isolation,heap,block")
    parser.add_argument("--bench-mib", type=int, default=4, help="MiB moved each way by the netbench suite")
    parser.add_argument("--bench-runs", type=int, default=1, help="netbench runs per offload setting")
    parser.add_argument("--tap", help="netbench suite over this tap interface (host address 10.0.2.2/24) instead of user networking")
    parser.add_argument("--asr-model", help="optional Vosk model directory (Russian) to check that tts speech is recognizable")
    args = parser.parse_args()
    suites = ["boot", "display", "net", "tls", "netbench", "tablet", "normal", "memory", "dzen", "services", "ahci", "audio", "tts", "listen", "keys", "shell", "tools", "windows", "wm", "vfs", "edit", "disk"] + (["busy", "smp"] if args.busy_elf else [])
    if args.isolation_elf:
        suites.append("isolation")
    if args.heap_elf:
        suites.append("heap")
    if args.block_elf:
        suites.append("block")
    if args.suites:
        suites = args.suites.split(",")
    for suite in suites:
        if suite == "block":
            block_suite(args, args.block_elf)
            continue
        if suite == "vfs":
            vfs_suite(args)
            continue
        if suite == "edit":
            edit_suite(args)
            continue
        if suite == "disk":
            disk_suite(args)
            continue
        with tempfile.TemporaryDirectory(prefix="smoke-", dir=ROOT / "usb_root") as temp:
            disk = Path(temp)
            (disk / "EFI/BOOT").mkdir(parents=True)
            for name in [*(p.name for p in (ROOT / "usb_root").glob("*.elf")), "EFI/BOOT/BOOTX64.EFI"]:
                shutil.copyfile(ROOT / "usb_root" / name, disk / name)
            shutil.copytree(ROOT / "usb_root/voice", disk / "voice")  # the voice recognizer's model and grammar
            if suite == "services":
                # Files the kernel and ABI know nothing about: only loader will find them.
                shutil.copyfile(disk / "clock.elf", disk / "hello.elf")
                (disk / "extra").mkdir()
                shutil.copyfile(disk / "app.elf", disk / "extra/demo.elf")
            if suite == "listen":
                speech, starts = speech_wav()
                (disk / "speech.wav").write_bytes(speech)
                (disk / "commands.wav").write_bytes(speech_wav(COMMANDS)[0])
                (disk / "voice.wav").write_bytes(speech_wav(DIALOGUE)[0])
                (disk / "docs").mkdir()
                (disk / "docs/notes.txt").write_text(NOTES, encoding="utf-8")  # read aloud by voice control
            if suite == "wm":
                (disk / "docs").mkdir()
                (disk / "docs/notes.txt").write_text(NOTES, encoding="utf-8")
            if suite == "tools":
                # caps without REQUEST_AUTHORITY (mind::process, 128): the request note patched in a copy.
                elf = bytearray((disk / "caps.elf").read_bytes())
                note = elf.index(b"MINDREQ1") + 8
                elf[note:note + 4] = (int.from_bytes(elf[note:note + 4], "little") & ~128).to_bytes(4, "little")
                (disk / "capsobs.elf").write_bytes(elf)
                (disk / "docs").mkdir()
                (disk / "docs/notes.txt").write_text(NOTES, encoding="utf-8")
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
            if suite == "display":
                display_suite(args, disk)
                continue
            if suite == "net":
                net_suite(args, disk)
                continue
            if suite == "tls":
                tls_suite(args, disk)
                continue
            if suite == "netbench":
                netbench_suite(args, disk)
                continue
            if suite == "tablet":
                tablet_suite(args, disk)
                continue
            wav = Path(tempfile.gettempdir()) / f"mind-core-{suite}.wav" if suite in ("audio", "tts") else "none" if suite == "listen" else None
            # The listen suite also has the launchers' network card: on QEMU's i440FX it shares the sound card's interrupt
            # line, and audio_gw must keep playing without interrupts (issue 096).
            vm = VM(args, disk.relative_to(ROOT).as_posix(),
                    rtc="2026-09-19T19:35:05" if suite == "dzen" else "localtime", audio=wav, ahci=suite == "ahci",
                    extra=["-nic", "user,model=virtio-net-pci"] if suite == "listen" else ())
            try:
                if suite == "audio":
                    audio_suite(vm, wav)
                elif suite == "tts":
                    tts_suite(vm, wav, args.asr_model)
                elif suite == "listen":
                    listen_suite(vm, starts)
                else:
                    {"normal": normal_suite, "busy": busy_suite, "memory": memory_suite,
                     "smp": smp_suite, "isolation": isolation_suite, "heap": heap_suite,
                     "dzen": dzen_suite, "services": services_suite, "ahci": ahci_suite, "keys": keys_suite, "shell": shell_suite, "tools": tools_suite, "windows": windows_suite, "wm": wm_suite}[suite](vm)
            finally:
                vm.close()
                log = Path(tempfile.gettempdir()) / f"mind-core-{suite}-{args.cpus}cpu.log"
                log.write_text(vm.log)
                print(f"QEMU log: {log}", flush=True)


if __name__ == "__main__":
    main()
