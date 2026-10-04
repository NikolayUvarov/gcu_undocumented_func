#!/usr/bin/env python3
"""Exercise the real bootloader/kernel/apps via QEMU's UART and HMP.

Uses only Python's standard library. QEMU may be a native binary or Windows QEMU
from WSL. Run after 02_build.sh; pass --qemu and --firmware as needed. Temporary
FAT roots are created below usb_root and removed, leaving the built OS intact.
"""
import argparse
import codecs
import math
import os
from pathlib import Path
import queue
import re
import shutil
import struct
import subprocess
import tempfile
import threading
import time

ROOT = Path(__file__).resolve().parents[1]
ANSI = re.compile(r"\x1b\[[0-9;?=]*[A-Za-z]")
# System services (PID 1..N, started by init); ahci/usb_storage drivers exist only when the controller is present.
SERVICES = ("init", "rtc", "ps2_kbd", "compositor", "ata", "ahci", "usb_storage", "ramdisk", "vfs_server", "loader", "audio_gw", "tts", "sysmon", "shell")
# Test suites number apps from 1; the harness maps their numbers to real PIDs (BASE is computed at boot).
BASE = 0
PID_IN = re.compile(r"\b(fg|kill|logs|pmap|stat|caps)(\s+)(\d{1,18})\b", re.I)
PID_OUT = re.compile(r"(PID[= ])(\d+)")


def to_real(text):
    return PID_IN.sub(lambda m: f"{m[1]}{m[2]}{int(m[3]) + BASE if int(m[3]) > 0 else m[3]}", text)


def to_ordinal(text):
    return PID_OUT.sub(lambda m: f"{m[1]}{int(m[2]) - BASE}" if int(m[2]) > BASE else m[0], text)


class VM:
    def __init__(self, args, disk, usb=False, rtc="localtime", audio=None, ahci=False, raw=False, snapshot=True):
        # `disk` is a directory served as a virtual FAT disk, or with `raw` (always for USB) a disk image; without
        # `snapshot` writes reach the image.
        self.disk = disk
        self.cpus = args.cpus
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
             "-serial", "mon:stdio", "-display", "none", "-rtc", f"base={rtc}", "-no-reboot",
             *(["-audiodev", "none,id=snd0" if audio == "none" else f"wav,id=snd0,path={audio}", "-device", "AC97,audiodev=snd0"] if audio else [])],
            cwd=ROOT, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
        )
        self.queue = queue.Queue()
        self.output = ""
        self.log = ""
        self.monitor = False
        threading.Thread(target=self._read, daemon=True).start()
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

    def send_bytes(self, data):
        for byte in data:
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
    registers = vm.hmp("info registers")
    require(registers, "HLT=1")
    cpus = vm.hmp("info cpus")
    assert len(re.findall(r"CPU #\d", cpus)) == vm.cpus, cpus
    vm.serial()
    assert heap_used(vm) == baseline
    print("PASS: instances, concurrent progress, fg, Ctrl+Z/UART+PS2, Esc, kill, logs, invalid input, limit/reuse, heap, HLT, 4 CPUs", flush=True)


def keys_suite(vm):
    """Key events: the same events from the UART (VT100 sequences, UTF-8) and from PS/2, layouts, Esc."""
    baseline = heap_used(vm)
    vm.send("run keys\n")
    vm.expect("[KEYS] READY")
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
    require(vm.service_logs("ps2_kbd", "[KBD] LAYOUT EN"), "[KBD] LAYOUT RU")
    assert task_rows(vm) == {}
    assert heap_used(vm) == baseline
    print("PASS: key events: VT100/xterm sequences and UTF-8 from the UART, E0 keys, F-keys and modifiers from PS/2, CRLF, Russian layout switch, Esc", flush=True)


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
    keys(b"ist\x1b[Hl\x1b[F\r", "PROGRAMS ON DISK:")
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


def status_line(vm, text, timeout=8, raw=False):
    # The whole line from `text` on (expect() may return before the line ends); `raw` keeps real PIDs.
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        vm.collect()
        clean = ANSI.sub("", vm.output).replace("\r", "")
        clean = clean if raw else to_ordinal(clean)
        at = clean.find(text)
        if at >= 0 and "\n" in clean[at:]:
            vm.output = ""
            return clean[at:clean.index("\n", at)]
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
    assert table_row(screen, re.escape(canon("Kernel arena 64.0M: used"))) and table_row(screen, canon(f"Tasks {tasks}/24")), screen
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
    for line in (r"0x0000008000000000 +\S+ +r-x +code", r"0x0000008001000000 +4\.0K +--- +guard", r"0x0000008001001000 +64\.0K +rw- +stack",
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
    for name in [f"CPU{cpu} " for cpu in range(vm.cpus)] + ["interrupts ", "syscalls ", "IPC messages ", "context switches ", "kernel arena ", f"tasks  {tasks} of 24"]:
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
    assert table_row(screen, r"IRQ 1 .* ps2_kbd \(PID \d+\)"), screen
    vm.send("q")
    require(vm.expect("EXITED. SHELL RESUMED."), "[HW] DONE")
    time.sleep(.1); vm.collect(); vm.output = ""
    require(vm.command(f"kill {clock}"), "KILLED")
    for _ in range(20):
        if heap_used(vm) == baseline:
            break
        time.sleep(.1)
    assert heap_used(vm) == baseline
    print("PASS: monitors: top (task table = ps, details, sorting, filter, tree), memmap (physical map, arena, a known address space, quotas), load (graphs, total, 10 min), hw (CPUID, framebuffer, PCI, IRQ holders)", flush=True)
    fm_check(vm)


def fm_check(vm):
    """The file manager: browse into EFI/BOOT and back, view a file, start a program from the panel."""
    baseline = heap_used(vm)
    def keys(data, text):
        vm.send_bytes(data)
        return status_line(vm, text)
    vm.send("fm\n")
    vm.expect("[FM] READY LEFT=/ FULL RIGHT=/ BRIEF ACTIVE=L CURRENT=docs")
    time.sleep(.3)
    screen = screen_text(vm)
    vm.serial()  # Enter: into docs
    assert canon("A:/") in screen[0] and canon("10Quit") in screen[-1], (screen[0], screen[-1])
    assert table_row(screen, r"║EFI +│.SUB-DIR.│\d{4}-\d\d-\d\d│\d\d:\d\d║"), screen
    assert table_row(screen, r"║kernel\.elf +│ +\d+│\d{4}-\d\d-\d\d│"), screen
    assert "CURRENT=.." in tool_status(vm, "[FM] LEFT=/docs FULL")
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
    assert "CURRENT=docs" in tool_status(vm, "[FM] LEFT=/ FULL")
    keys(b"\x1b[B", "CURRENT=EFI")
    keys(b"\r", "LEFT=/EFI FULL")
    keys(b"\x1b[B", "CURRENT=BOOT")
    keys(b"\r", "LEFT=/EFI/BOOT FULL")
    tool_status(vm, "LEFT=/EFI/BOOT FULL")  # not CR last: CR LF would be one Enter
    time.sleep(.2)
    screen = screen_text(vm)
    vm.serial()  # Enter on "..": back to EFI
    assert canon("A:/EFI/BOOT") in screen[0] and table_row(screen, r"║BOOTX64\.EFI +│ +\d+│"), screen
    assert "CURRENT=BOOT" in tool_status(vm, "[FM] LEFT=/EFI FULL")
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
    print("PASS: fm: two panels with sizes and dates, the built-in viewer, EFI/BOOT and back, a program started from the panel", flush=True)
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
    require(vm.command("logs 1"), "BUSY FIXTURE")
    require(vm.command("kill 1"), "KILLED PID=1")
    vm.command("kill 2")
    assert heap_used(vm) == baseline
    print("PASS: timer preemption of a non-yielding SIMD loop; responsive shell, clocks and kill; top shows the loop at ~100 % of its CPU", flush=True)


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
             ("d", 14, 4)]
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
    assert int(task_rows(vm)[1][-1]) > int(before[-1])
    vm.command("kill 1")
    assert heap_used(vm) == baseline, "fault teardown leaked task/page-table resources"
    require(vm.command("run app &"), "NAME=app BACKGROUND")
    print("PASS: CPL3/IOPL0; kernel read/write, RX code, NX stack, CLI/I/O, UD2, guard/bad stack; syscall pointers; capability checks and endpoint badges; fault containment and reclaim", flush=True)


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
    print("PASS: AHCI driver in ring 3 (MMIO + DMA capabilities), VFS mounted from SATA, file reads", flush=True)


def services_suite(vm):
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
    assert f"TASKS={tasks}/24" in free, (tasks, free)
    arena, used, largest = map(int, re.search(r"ARENA=(\d+) USED=(\d+) FREE=\d+ LARGEST=(\d+)", free).groups())
    assert arena == 64 << 20 and 0 < used < arena and 0 < largest <= arena - used, free
    cpus = vm.command("cpus")
    assert len(re.findall(r"BUSY_MS=\d+ IDLE_MS=\d+ SWITCHES=\d+", cpus)) == vm.cpus, cpus
    physmap = vm.command("physmap")
    for kind in ("free RAM", "kernel arena", "kernel", "framebuffer", "boot image"):
        require(physmap, kind)
    free_ram = int(re.search(r"FREE_RAM=(\d+)K", physmap)[1])
    assert 128 * 1024 < free_ram < 512 * 1024, free_ram  # the VM has 512 MiB
    require(vm.command("irqs"), f"IRQ=1 COUNT=")
    require(vm.command("devices"), "00:01.1 010180 IDE controller")
    endpoints = vm.command("endpoints")
    assert len(re.findall(r"^EP=\d+ CREATOR=1 SERVER=\d+", endpoints, re.M)) >= 6, endpoints
    require(vm.service_logs("sysmon", "[SYSMON] READY"), "[SYSMON] READY: SAMPLES EVERY 100 MS")
    # Calendar date from the rtc service (idl/rtc.wit 1.1): QEMU's RTC follows the host's local time here.
    import datetime
    today = datetime.date.today()
    date = vm.command("date")
    assert any(f"DATE: {d.isoformat()} " in date for d in (today, today - datetime.timedelta(days=1), today + datetime.timedelta(days=1))), date
    require(vm.command("fg -4"), "ERROR:")  # the harness does not translate negative numbers
    vm.send("fg 0\n"); vm.expect("ERROR:")
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
    require(vm.command("run hello &"), "PID=4 NAME=hello BACKGROUND")
    # The address space of a known program (hello is clock.elf) as STAT_VMAP reports it: the layout paging.rs sets up.
    pmap = vm.command("pmap 4")
    require(pmap, "0x0000008000000000 ")
    assert re.search(r"0x0000008000000000 +\d+ r-x code", pmap), pmap
    for line in ("0x0000008001000000      4096 --- guard", "0x0000008001001000     65536 rw- stack", "0x0000008001011000      4096 --- guard",
                 "0x0000008004000000      4096 r-- info", "0x0000008004001000      4096 rw- mailbox", "0x0000008005000000      4096 r-x exit"):
        require(pmap, line)
    assert re.search(r"0x0000008002000000 +\d+ rw- screen", pmap), pmap
    details = vm.command("stat 4")
    require(details, "NAME=hello")
    require(details, "QUOTA TASKS=0/0 ENDPOINTS=0/4")
    require(details, "CAPS=5/31")
    # The standard client endpoints in slots 2..6, write and grant only; no privilege.
    caps = vm.command("caps 4")
    for slot in (2, 3, 4, 5, 6):
        assert re.search(fr"SLOT={slot} GEN=0 endpoint NODE=\d+ PARENT=\d+ EP=\d+ RIGHTS=-wg-", caps), (slot, caps)
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
    # No endpoint numbers: a restarted service gets a new receiver of the endpoint init keeps, so a client granted
    # earlier reaches it again; while it is dead, calls fail instead of hanging.
    require(vm.command(f"kill {vm.services()['rtc']}", raw=True), "KILLED PID=")
    require(vm.command("run hello &"), "PID=6 NAME=hello BACKGROUND")
    time.sleep(1)
    assert "[CLOCK] " not in vm.command("logs 6"), "a dead RTC service must not answer"
    require(vm.command("run rtc &"), "NAME=rtc")
    output = ""
    for _ in range(20):
        output += vm.command("logs 6")
        if "[CLOCK] " in output:
            break
        time.sleep(.2)
    require(output, "[CLOCK] ")
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
    print("PASS: boot services, monotonic clock, single instances, IPC call/reply with memory caps, peer death, VFS list/read over ATA driver + FAT, programs loaded from disk by loader, service restart for existing clients, reclaim, launch sessions with requested capabilities, console programs", flush=True)


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
        finally:
            vm.close()
            (Path(tempfile.gettempdir()) / f"mind-core-vfs-1-{args.cpus}cpu.log").write_text(vm.log)
        fsck_volume(image, start, fs_sectors)
        mtype = lambda name: subprocess.run(["mtype", "-i", part, f"::/{name}"], env=MTOOLS_ENV, capture_output=True).stdout
        assert mtype("data/notes.txt") == b"line one\n", mtype("data/notes.txt")
        assert mtype("data/b.txt") == b"alpha\n"
        assert b"sub" not in subprocess.run(["mdir", "-b", "-i", part, "::/data"], env=MTOOLS_ENV, capture_output=True).stdout
        # After a reboot: the disk keeps its files, the RAM disk starts empty.
        subprocess.run(["mdel", "-i", part, "::/NvVars"], env=MTOOLS_ENV, capture_output=True)
        vm = VM(args, image.relative_to(ROOT).as_posix(), raw=True, snapshot=False)
        try:
            require(vm.command("cat data/notes.txt"), "line one")
            require(vm.command("ls data"), "2 ENTRIES, 2 FILES, 15 BYTES")
            require(vm.command("ls ram:"), "0 ENTRIES")
        finally:
            vm.close()
            (Path(tempfile.gettempdir()) / f"mind-core-vfs-2-{args.cpus}cpu.log").write_text(vm.log)
        fsck_volume(image, start, fs_sectors)
    print("PASS: vfs: files written to a raw FAT disk in data/ pass fsck.fat and read back with mtools and after a reboot; the RAM disk is empty after it", flush=True)


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
    leave(f10, "[EDIT] DONE")
    # A file with CR LF line endings keeps them; a new line gets one too.
    assert "BYTES=15 LINES=3 MODIFIED=0" in start("data/crlf.txt")
    keys(ctrl_end, "LINE=3 COL=1 ")
    keys("три".encode(), "BYTES=21 ")
    keys(b"\r", "LINE=4 COL=1 BYTES=23 ")
    keys(b"4", "BYTES=24 ")
    keys(f2, "[EDIT] SAVED 24 BYTES TO data/crlf.txt")
    leave(f10, "[EDIT] DONE")
    # A boot file opens read-only: typing changes nothing; Shift+F2 saves a copy on ram:, which may be changed.
    assert "BYTES=33 LINES=2 MODIFIED=0 DIALOG=NONE MENU=0 RO=1" in start("readme.txt")
    keys(b"x", "BYTES=33 LINES=2 MODIFIED=0")
    keys(shift_f2, "DIALOG=SAVEAS")
    keys(b"\x7f" * 10, "DIALOG=SAVEAS")
    keys(b"ram:copy.txt", "DIALOG=SAVEAS")
    keys(b"\r", "[EDIT] SAVED 33 BYTES TO ram:copy.txt")
    keys(b"x", "BYTES=34 ")
    keys(f10, "DIALOG=UNSAVED")
    keys(b"\x1b[C", "DIALOG=UNSAVED")  # Right: Don't save
    leave(b"\r", "[EDIT] DONE")
    require(utf8("cat ram:copy.txt"), "Только для чтения")
    require(vm.command("ls data"), "2 ENTRIES, 2 FILES, 59 BYTES")
    print("PASS: edit: Latin and Cyrillic text saved on ram: and in data/ (F2, the unsaved-changes dialog), read back; CRLF kept; a boot file opens read-only and is saved elsewhere", flush=True)


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
                assert "ATTACH=OK WRITE=OK FLUSH=OK READBACK=OK" in line, line
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
    require(vm.command("run say -p 150 -r 120 hello world &"), "PID=2 NAME=say BACKGROUND")
    log = ""
    for _ in range(40):
        log += vm.command("logs 2")
        if "[SAY] DONE" in log:
            break
        time.sleep(.25)
    assert re.search(r"\[SAY\] SPOKE \d+ MS", log), log
    vm.command("kill 2")
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


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--qemu", default=os.environ.get("QEMU", "qemu-system-x86_64"))
    parser.add_argument("--cpus", type=int, default=4)
    parser.add_argument("--firmware", default="OVMF.fd")
    parser.add_argument("--busy-elf", help="test-only ELF built from tests/busy_app.rs")
    parser.add_argument("--isolation-elf", help="test-only ELF built from tests/isolation_app.rs")
    parser.add_argument("--heap-elf", help="test-only ELF built from tests/heap_app.rs")
    parser.add_argument("--block-elf", help="test-only ELF built from tests/block_app.rs (stands in for vfs_server)")
    parser.add_argument("--suites", help="comma-separated subset: normal,memory,dzen,services,ahci,audio,tts,listen,keys,shell,tools,vfs,edit,busy,smp,isolation,heap,block")
    parser.add_argument("--asr-model", help="optional Vosk model directory (Russian) to check that tts speech is recognizable")
    args = parser.parse_args()
    suites = ["normal", "memory", "dzen", "services", "ahci", "audio", "tts", "listen", "keys", "shell", "tools", "vfs", "edit"] + (["busy", "smp"] if args.busy_elf else [])
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
            if suite == "tools":
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
                     "dzen": dzen_suite, "services": services_suite, "ahci": ahci_suite, "keys": keys_suite, "shell": shell_suite, "tools": tools_suite}[suite](vm)
            finally:
                vm.close()
                log = Path(tempfile.gettempdir()) / f"mind-core-{suite}-{args.cpus}cpu.log"
                log.write_text(vm.log)
                print(f"QEMU log: {log}", flush=True)


if __name__ == "__main__":
    main()
