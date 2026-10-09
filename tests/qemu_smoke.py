#!/usr/bin/env python3
"""Exercise the real bootloader/kernel/apps via QEMU's UART and HMP.

Uses only Python's standard library. QEMU may be a native binary or Windows QEMU
from WSL. Run after 02_build.sh; pass --qemu and --firmware as needed. Temporary
FAT roots are created below usb_root and removed, leaving the built OS intact.
"""
import argparse
import base64
import codecs
import hashlib
import json
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
import sys
import tempfile
import threading
import time
import wave

ROOT = Path(__file__).resolve().parents[1]
# Every boot volume the harness builds is signed as the build signs usb_root (350-UPD-0002): the bootloader loads
# nothing its manifest does not describe.
sys.path.insert(0, str(ROOT / "scripts"))
import sign_manifest  # noqa: E402
import boot_slots_check  # noqa: E402
import boot_slots  # noqa: E402
import serve_release  # noqa: E402
sys.path.insert(0, str(ROOT / "scripts" / "voice_dictate"))
from fbank_reference import signal as fbank_signal  # noqa: E402
FBANK_SIGNAL = fbank_signal()  # the integer test signal of tests/fbank_reference.txt (250)
ANSI = re.compile(r"\x1b\[[0-9;?=]*[A-Za-z]")
# System services (PID 1..N, started by init); ahci/usb_storage/virtio_blk/virtio_net/virtio_input exist only when their device is present.
SERVICES = ("init", "logd", "rtc", "ps2_kbd", "virtio_input", "compositor", "ata", "ahci", "usb_host", "usb_storage", "usb_hid", "virtio_blk", "nvme", "ramdisk", "ramdisk#1", "vfs_server", "blockstore", "gpio", "loader", "audio_gw", "tts", "video_gw", "virtio_net", "virtio_net#1", "netstack", "netpolicy", "parse", "tpm", "keystore", "tls", "windows", "sysmon", "updater", "shell")
RECOVERY_RESERVE = 32 * 1024 * 1024  # init's RECOVERY_RESERVE_MIB: frames applications may not take (issue 169)
# The built image the suites boot (usb_root, or aarch64_root with --arch aarch64) and its UEFI boot file.
IMAGE = "usb_root"
BOOT_EFI = "EFI/BOOT/BOOTX64.EFI"
# The boot disk's driver, as vfs names it and as a service.
BOOT_DRIVE, BOOT_DRIVER = "ATA", "ata"
# Test suites number apps from 1; the harness maps their numbers to real PIDs (BASE is computed at boot).
BASE = 0
PID_IN = re.compile(r"\b(fg|kill|logs|pmap|stat|caps|budget)(\s+)(\d{1,18})\b", re.I)
PID_OUT = re.compile(r"(PID[= ])(\d+)")


def to_real(text):
    return PID_IN.sub(lambda m: f"{m[1]}{m[2]}{int(m[3]) + BASE if int(m[3]) > 0 else m[3]}", text)


def to_ordinal(text):
    return PID_OUT.sub(lambda m: f"{m[1]}{int(m[2]) - BASE}" if int(m[2]) > BASE else m[0], text)


class VM:
    def __init__(self, args, disk, usb=False, rtc="localtime", audio=None, ahci=False, raw=False, snapshot=True, prompt=True, extra=(), reboot=False, tablet=False, usb_input=False, decoy=None, audio_card="AC97"):
        # `disk` is a directory served as a virtual FAT disk, or with `raw` (always for USB) a disk image; without
        # `snapshot` writes reach the image. `tablet`: a VirtIO tablet, driven through the QMP socket (`tablet_at`).
        # Monitor commands go through the QMP socket too (`hmp`): typed into the monitor on the serial line, its echo
        # and line ends came in the middle of lines the guest printed.
        # `decoy`: a directory served as another FAT disk ahead of the boot disk, on the same bus (211-KRN-0012).
        self.disk = disk
        self.qmp_path = Path(tempfile.mkdtemp(prefix="mind-qmp-")) / "qmp.sock"
        self.qmp_file = None
        self.temporary = []
        self.monitor_used, self.monitor_cpu = False, None
        extra = (*extra, "-qmp", f"unix:{self.qmp_path},server=on,wait=off")
        if tablet:
            extra = (*extra, "-device", "virtio-tablet-pci")
        if usb_input:
            # USB input only (issue 164): a keyboard behind a hub, a tablet on a root port; no PS/2 or VirtIO input.
            extra = (*extra, "-device", "qemu-xhci,id=xhci", "-device", "usb-hub,bus=xhci.0,port=1,id=hub",
                     "-device", "usb-kbd,bus=xhci.0,port=1.1,id=kbd", "-device", "usb-tablet,bus=xhci.0,port=2,id=usbtablet")
        self.cpus, self.args = args.cpus, args
        filename = disk.replace(",", ",,")
        source = f"format=raw,file={filename}" if usb or raw else f"format=raw,file=fat:{filename}"
        storage = (["-drive", f"{source},if=none,id=usbdisk",
                    "-device", "qemu-xhci", "-device", "usb-storage,drive=usbdisk,bootindex=1,id=usbstick"]
                   if usb else ["-drive", f"{source},if=none,id=sata",
                                "-device", "ahci,id=ahci", "-device", "ide-hd,drive=sata,bus=ahci.0"]
                   if ahci else ["-drive", f"{source},if=none,id=nvm", "-device", "nvme,serial=mind,drive=nvm"]
                   if getattr(args, "disk", None) == "nvme" else ["-drive", source])
        if decoy:
            storage = ["-drive", f"format=raw,file=fat:{decoy.replace(',', ',,')}", *storage]
        self.arch = getattr(args, "arch", "x86_64")  # usb_image_smoke.py passes no architecture
        if self.arch == "aarch64":
            # QEMU virt (issue 202): AAVMF in pflash with its own variable store, the ECAM below 4 GiB, ramfb for the
            # GOP framebuffer, a VirtIO keyboard (sendkey) and tablet; the boot disk is a VirtIO block device.
            variables = Path(tempfile.mkdtemp(prefix="mind-vars-")) / "vars.fd"
            self.temporary.append(variables.parent)  # 64 MiB a boot: removed in close
            shutil.copyfile(args.aavmf_vars, variables)
            machine = ["-machine", getattr(args, "machine", None) or "virt,gic-version=3,highmem=off", *([] if "-cpu" in extra else ["-cpu", "max"]),
                       "-drive", f"if=pflash,format=raw,readonly=on,file={args.aavmf_code}", "-drive", f"if=pflash,format=raw,file={variables}",
                       "-device", "ramfb", *([] if usb_input else ["-device", "virtio-keyboard-pci"]), *([] if "virtio-tablet-pci" in extra or usb_input else ["-device", "virtio-tablet-pci"])]
        else:
            machine = ["-bios", args.firmware, *(["-machine", "pc,i8042=off"] if usb_input else []), *(["-machine", m] if (m := getattr(args, "machine", None)) else [])]
        self.process = subprocess.Popen(
            [args.qemu, *machine, *storage,
             *(["-snapshot"] if snapshot else []), "-m", getattr(args, "memory", None) or "512", "-smp", f"{args.cpus},sockets=1,cores={args.cpus},threads=1",
             "-serial", "mon:stdio", "-display", "none", "-rtc", f"base={rtc}", *([] if reboot else ["-no-reboot"]), *extra,
             *(["-cpu", model] if (model := getattr(args, "cpu_model", None)) and "-cpu" not in extra else []),
             *(["-audiodev", "none,id=snd0" if audio == "none" else f"wav,id=snd0,path={audio}",
                *(["-device", "intel-hda", "-device", "hda-duplex,audiodev=snd0"] if audio_card == "HDA" else ["-device", "AC97,audiodev=snd0"])] if audio else [])],
            cwd=ROOT, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
        )
        self.queue = queue.Queue()
        self.output = ""
        self.log = ""
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

    def qmp(self, command, **arguments):
        # One QMP command (the tablet's events: input-send-event); the reply.
        if self.qmp_file is None:
            connection = socket.socket(socket.AF_UNIX)
            for _ in range(100):  # QEMU creates the socket as it starts
                try:
                    connection.connect(str(self.qmp_path))
                    break
                except (FileNotFoundError, ConnectionRefusedError):
                    time.sleep(.05)
            self.qmp_file = connection.makefile("rw")
            self.qmp_file.readline()  # the greeting
            self.qmp("qmp_capabilities")
        self.qmp_file.write(json.dumps({"execute": command, **({"arguments": arguments} if arguments else {})}) + "\n")
        self.qmp_file.flush()
        while True:
            reply = json.loads(self.qmp_file.readline())
            if "return" in reply or "error" in reply:
                assert "error" not in reply, reply
                return reply["return"]

    def tablet_at(self, x, y, width=1280, height=800):
        # The host's pointer at pixel (x, y) of the screen, through the tablet (0..32767 across it).
        self.qmp("input-send-event", events=[{"type": "abs", "data": {"axis": "x", "value": (2 * x + 1) * 32768 // (2 * width)}},
                                             {"type": "abs", "data": {"axis": "y", "value": (2 * y + 1) * 32768 // (2 * height)}}])

    def tablet_click(self, button="left", wait=.08):
        for down in (True, False):
            self.qmp("input-send-event", events=[{"type": "btn", "data": {"down": down, "button": button}}])
            time.sleep(wait)

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

    def program_logs(self, pid, until, tries=80):
        """What program `pid` logged, until `until` is in it. `logs` drains the log, and a line printed in pieces may
        be drained half at a time ("[MEMTEST] HELD " in one, "144 MiB INTACT=true" in the next), so bodies are joined."""
        text = ""
        for _ in range(tries):
            output = self.command(f"logs {pid}").replace("\r", "")
            body = re.search(r"LOGS PID=\d+ \([^)]*\):\n(.*)\nEND LOGS", output, re.S)
            text += body[1] if body else output
            if until in text:
                break
            time.sleep(.25)
        return text

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
        if not raw:
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
        return self.expect("MIND> ", after=to_ordinal(text if raw else to_real(text)) + "\n")

    def hmp(self, command):
        # A monitor command through QMP (human-monitor-command); what it printed. `cpu N` picks the CPU later
        # commands look at, as it does in the monitor. Paced as typing it into the monitor at 10 ms a byte was (and
        # entering the monitor first): the suites' waits grew around that time.
        time.sleep(.01 * (len(command) + 1) + (0 if self.monitor_used else .03))
        self.monitor_used = True
        if cpu := re.fullmatch(r"cpu (\d+)", command):
            self.monitor_cpu = int(cpu[1])
            return ""
        return self.qmp("human-monitor-command", **{"command-line": command}, **({} if self.monitor_cpu is None else {"cpu-index": self.monitor_cpu}))

    def serial(self, enter=True):
        # After monitor commands: Enter reaches the program in front, as it did when leaving the monitor on the
        # serial line (`enter=False` leaves it out); what came meanwhile is not waited for.
        if self.monitor_used:
            if enter:
                self.send("\n")
            self.monitor_used = False
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
            # Relative inside the tree (no drive letter for HMP on Windows); a disk elsewhere (/tmp) is named in full.
            self.hmp(f"screendump {(path.relative_to(ROOT) if path.is_relative_to(ROOT) else path).as_posix()}")
            data = path.read_bytes()
            # Retain a viewable artifact outside the build tree.
            (Path(tempfile.gettempdir()) / "mind-core-clock.ppm").write_bytes(data)
            return data
        finally:
            path.unlink(missing_ok=True)

    def close(self):
        if self.qmp_file is not None:
            self.qmp_file.close()
        if self.process.poll() is None:
            self.process.terminate()
            self.process.wait(timeout=10)
        shutil.rmtree(self.qmp_path.parent, ignore_errors=True)
        for path in self.temporary:
            shutil.rmtree(path, ignore_errors=True)
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
    # mind::tui centers the text grid: a screen not a multiple of the cell (800x600 on aarch64) has margins.
    x0, y0 = width % 8 // 2, height % 16 // 2
    lines = []
    for cy in range(height // 16):
        line = []
        for cx in range(width // 8):
            rows = [[pixels[((y0 + cy * 16 + r) * width + x0 + cx * 8 + c) * 3:((y0 + cy * 16 + r) * width + x0 + cx * 8 + c) * 3 + 3] for c in range(8)] for r in range(16)]
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
    """Starts memtest instances that hold all but about `leave_mib` of what applications may take of the frame pool
    (the recovery reserve stays for services, issue 169); returns their PIDs."""
    holders, left = [], frames - leave_mib * 1024 * 1024 - RECOVERY_RESERVE
    while (mib := min(144, left // (1024 * 1024)) // 16 * 16) > 0:
        output = vm.command(f"run memtest hold {mib} &")
        holders.append(int(re.search(r"STARTED PID=(\d+)", output)[1]))
        if f"HELD {mib} MiB" not in vm.program_logs(holders[-1], f"HELD {mib} MiB"):
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
    if not getattr(vm, "settled", False):
        # The first reading is a suite's baseline. The services' start goes on after the first prompt, and the kernel frees
        # what it held as it ends: its list of revoked memory once nothing references that memory (224 bytes), a 4 KiB
        # memory object, and 4320 bytes of arena while the block store mounts its medium (16 CPUs on aarch64, up to 3.6 s
        # after boot). The baseline waits for no memory object to be pending and for the block store to wait for requests
        # (read from `stat`: `logs` would drain the lines later checks look for), then until readings half a second apart
        # agree.
        vm.settled = True
        for _ in range(40):
            if re.search(r"OBJECTS=0/", vm.command("free")):
                break
            time.sleep(.25)
        store = vm.services().get("blockstore")
        for _ in range(120):
            if store is None or re.search(r"STATE=RECV ", vm.command(f"stat {store}", raw=True)):
                break
            time.sleep(.25)
        used = heap_used(vm)
        for _ in range(10):
            time.sleep(.5)
            again = heap_used(vm)
            if again == used:
                break
            used = again
        return used
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
    # Apps only: services are visible in ps, but the suites check user tasks (CONSOLE=n: the shell's console that
    # started it, issue 155).
    return {int(m[0]) - BASE: m[1:] for m in re.findall(
        r"^(-?\d+) ([\w-]+) (READY|RUNNING|SLEEPING|EXITED|IPC_WAIT|IRQ_WAIT) (BG|FG) (\d+) (\d+) (\d+) (\d+)(?: CONSOLE=\d)?$", output, re.M)
        if m[1] not in SERVICES}


def center_pixel(vm):
    _, size, _, pixels = vm.screenshot().split(b"\n", 3)
    width, height = map(int, size.split())
    at = ((height // 2) * width + width // 2) * 3
    return pixels[at:at + 3]


# aarch64 idle check (issue 208): an idle virt with 4 CPUs uses about 0.45-0.6 s of host processor time a second
# under TCG, one vCPU that spins instead of waiting in WFI about 1 s on its own. The lowest of a few samples is
# compared, so one sample that meets a busy host or a burst of wakeups does not fail the check.
IDLE_LIMIT = .85


def idle_cpu_seconds(vm, samples=3):
    return min(qemu_cpu_seconds(vm, 1) for _ in range(samples))


def qemu_cpu_seconds(vm, seconds):
    """Processor time QEMU used in `seconds` of wall time (user and system, all its threads)."""
    def total():
        fields = Path(f"/proc/{vm.process.pid}/stat").read_text().rpartition(")")[2].split()
        return (int(fields[11]) + int(fields[12])) / os.sysconf("SC_CLK_TCK")
    start = total(); time.sleep(seconds)
    return total() - start


def gibibytes(memory):
    # QEMU -m as GiB: "6G", "6144M" or "6144" (MiB).
    number, unit = re.fullmatch(r"(\d+)([GgMm]?)", memory or "512").groups()
    return int(number) / (1 if unit in "Gg" and unit else 1024)


def applications_until_memory_ends(vm, others=0):
    """Issue 171: no fixed count of applications. Clocks, each with its screen, start until the frame pool runs out, far
    past the 32 tasks the kernel's table used to hold; the refusal is clean and the system goes on. With more than 40
    tasks, uptime and top count every one (171-APP-0002). Once they end, the frame pool is back where it was, and the
    kernel arena too or at a level that a second round does not raise (the kernel may keep capacity a peak grew)."""
    baseline, frames = heap_used(vm), frames_free(vm)
    def clocks(most):
        pids = []
        while len(pids) < most:
            output = vm.command("run clock &")
            started = re.search(r"PID=(\d+) NAME=clock BACKGROUND", output)
            if not started:
                require(output, "OUT OF MEMORY")
                break
            pids.append(int(started[1]))
        return pids
    def settled(arena):
        for _ in range(40):
            if heap_used(vm) == arena and frames_free(vm) == frames:
                return True
            time.sleep(.25)
        return False
    pids = clocks(1000)
    assert len(pids) > 32, len(pids)
    # 171-KRN-0032: the frame pool, not the kernel arena, ran out: what is left is under the recovery reserve and one
    # more clock, and each clock took little arena (its task, tables and capabilities are in the frame pool).
    at_peak, arena = frames_free(vm), heap_used(vm) - baseline
    assert at_peak < (48 << 20), f"{at_peak >> 20} MiB of the frame pool left at the refusal"
    assert arena < len(pids) * 4096, f"{arena} bytes of arena for {len(pids)} clocks"
    assert len(task_rows(vm)) == len(pids) + others, (len(task_rows(vm)), len(pids), others)  # others: programs started before
    assert re.search(r"\d{4}-\d\d-\d\d", vm.command("date")), "the system goes on after the refusal"
    peak = len(pids)
    # 45 stay for the monitors: a hundred and more clocks drawing every second leave an emulated machine little time.
    for pid in pids[45:]:
        vm.command(f"kill {pid}")
    pids = pids[:45]
    time.sleep(1)
    running = len(re.findall(r"^\d+ [\w#-]+ [A-Z_]+ (?:BG|FG) ", vm.command("ps", raw=True), re.M))
    sampled = int(re.search(r"(\d+) tasks", vm.command("uptime"))[1])
    assert sampled > 40 and abs(sampled - running) <= 1, (sampled, running)
    vm.send("top\n")
    vm.expect("[TOP] READY")
    time.sleep(1.5)
    rows = int(re.search(r"ROWS=(\d+)", tool_status(vm, "[TOP] SORT="))[1])
    vm.send("q")
    require(vm.expect("EXITED. SHELL RESUMED."), "[TOP] DONE")
    time.sleep(.1); vm.collect(); vm.output = ""
    assert rows > 40 and rows >= running, (rows, running)
    for pid in pids:
        vm.command(f"kill {pid}")
    kept = ""
    if not settled(baseline):
        retained = heap_used(vm)
        assert frames_free(vm) == frames, f"frame pool {frames_free(vm)} (was {frames})"
        # Capacity the peak grew, or a leak: a second round of 20 would raise a leak again.
        for pid in clocks(20):
            vm.command(f"kill {pid}")
        assert settled(retained), f"arena {heap_used(vm)} (was {retained} after the first round, {baseline} before), frame pool {frames_free(vm)} (was {frames})"
        kept = f" ({retained - baseline} bytes of arena kept from the peak, not raised by a second round)"
    print(f"PASS: {peak} clocks at once until the frame pool ran out ({at_peak >> 20} MiB left, {arena // peak} bytes of arena a clock), a clean refusal; uptime and top see all {rows} tasks; frame pool and arena back{kept}", flush=True)


def monitors_every_cpu(vm):
    """171-APP-0007: with more CPUs than the former 8, top draws a bar for every CPU and load a graph for each of the 16
    a sample keeps."""
    def screen_of(program):
        vm.send(f"{program}\n")
        vm.expect(f"[{program.upper()}] READY")
        time.sleep(2.5)  # two refreshes: top's shares since the last one
        screen = screen_text(vm)
        vm.serial(enter=False)
        vm.send("q")
        require(vm.expect("EXITED. SHELL RESUMED."), f"[{program.upper()}] DONE")
        time.sleep(.1); vm.collect(); vm.output = ""
        return "\n".join(screen)
    bars = {int(c) for c in re.findall(r"CPU(\d+) +\[", screen_of("top"))}
    assert bars == set(range(vm.cpus)), bars
    graphs = {int(c) for c in re.findall(r"CPU(\d+)  \d+\.\d%", screen_of("load"))}
    assert graphs == set(range(min(vm.cpus, 16))), graphs
    print(f"PASS: top shows {len(bars)} CPUs, load {len(graphs)} CPU graphs", flush=True)


def clocks_on_many_cpus(vm):
    """171-KRN-0008: with more than 8 CPUs, 40 clocks with screens, past the 25 after which the shell stopped answering
    while wake IPIs were sent again and again under the scheduler lock. Every command is answered within the harness's
    wait, the clocks run on more than 8 CPUs, and the system answers once they are killed."""
    pids = []
    for _ in range(40):
        started = re.search(r"PID=(\d+) NAME=clock BACKGROUND", vm.command("run clock &"))
        assert started, "a clock starts"
        pids.append(int(started[1]))
    clocks = {pid: row for pid, row in task_rows(vm).items() if row[0] == "clock"}
    assert len(clocks) == len(pids), (len(clocks), len(pids))
    cpus = {int(row[3]) for row in clocks.values()}
    assert len(cpus) > 8, cpus
    for pid in pids:
        require(vm.command(f"kill {pid}"), f"KILLED PID={pid}")
    assert not any(row[0] == "clock" for row in task_rows(vm).values()), "every clock ended"
    assert re.search(r"\d{4}-\d\d-\d\d", vm.command("date")), "the system answers"
    print(f"PASS: 40 clocks on {len(cpus)} of {vm.cpus} CPUs, every command answered within the harness's wait", flush=True)


def pool_covers_free_ram(vm):
    """Issue 171 (171-KRN-0005): the frame pool takes every free range of the firmware map (at least 2 MiB, from 1 MiB
    up; above 4 GiB on x86 whole 2 MiB pages), however many there are: its size is their sum."""
    expected, ranges = 0, 0
    for start, end in re.findall(r"^(0x[0-9a-f]+)-(0x[0-9a-f]+) +\d+K free RAM$", vm.command("physmap"), re.M):
        start, end = int(start, 16), int(end, 16) + 1
        start = -(-max(start, 0x100000) // 4096) * 4096
        pieces = [(start, min(end, 1 << 32))] if vm.arch == "x86_64" else [(start, min(end, 1 << 40))]
        if vm.arch == "x86_64":
            pieces.append((-(-max(start, 1 << 32) // (2 << 20)) * (2 << 20), min(end, 1 << 39) // (2 << 20) * (2 << 20)))
        for a, b in pieces:
            if b > a and b - a >= 2 << 20:
                expected, ranges = expected + b - a, ranges + 1
    total = int(re.search(r"FRAMES=(\d+) FRAMES_FREE=", vm.command("free"))[1])
    assert total == expected, (total, expected, ranges)
    print(f"PASS: the frame pool is every free range of the firmware map ({ranges} ranges, {total >> 20} MiB)", flush=True)
    return ranges


def ram_above_4g(vm):
    """Issue 171 (171-KRN-0001): the frame pool takes the free RAM above 4 GiB too, the highest range first; a
    program's heap from it is written and read back whole."""
    physmap = vm.command("physmap")
    high = sum(max(0, int(last, 16) + 1 - max(int(start, 16), 1 << 32))
               for start, last in re.findall(r"^0x([0-9a-f]+)-0x([0-9a-f]+) +\d+K free RAM$", physmap, re.M))
    assert high >= (gibibytes(vm.args.memory) - 4) * (1 << 30), physmap
    frames = int(re.search(r"FRAMES=(\d+) FRAMES_FREE=", vm.command("free"))[1])
    # The pool has every 2 MiB page of it (the x86 kernel maps RAM above 4 GiB in 2 MiB pages) besides the RAM below.
    assert frames >= high - (4 << 20) and frames > 4 << 30, (frames, high)
    vm.send("memtest alloc 144\n")
    output = vm.expect("MIND> ", timeout=120, after="memtest alloc 144\n")
    require(output, "[MEMTEST] HELD 144 MiB INTACT=true")
    require(output, "[MEMTEST] FREED")
    print(f"PASS: {frames >> 20} MiB in the frame pool, {high >> 20} MiB of it above 4 GiB; a 144 MiB heap from the highest range written and read back", flush=True)


def hardware_report_check(vm):
    """174-KRN-0038: init asks the kernel for the hardware report at boot; without a log volume it writes nothing (ram:
    and data/ are the user's) and says how large the report was. usb_image_smoke.py reads one from a log volume."""
    log = ""
    for _ in range(60):
        log += vm.command("logs 1", raw=True)
        if "[INIT] HARDWARE REPORT" in log:
            break
        time.sleep(.5)
    size = re.search(r"\[INIT\] HARDWARE REPORT: NO LOG VOLUME, NOT WRITTEN \((\d+) BYTES\)", log)
    assert size and int(size[1]) > 4000, log[-2000:]


def normal_suite(vm):
    # No pin controller on QEMU (virt with ACPI has none, issue 206): gpio is not started (issue 207).
    assert "gpio" not in vm.services()
    if vm.arch == "aarch64":
        require(vm.log, "[INIT] gpio NOT STARTED: NO DEVICE")  # init's lines reach the serial line there
    else:
        # 211-PRT-0003: the tick from the LAPIC timer, measured on the ACPI PM timer (QEMU's chipsets have one).
        tick = re.search(r"MIND CORE KERNEL: TICK: LAPIC TIMER, \d+ PER TICK, MEASURED ON THE ACPI PM TIMER; TSC \d+ MHZ; PIT (NOT )?COUNTING\n",
                         ANSI.sub("", vm.log).replace("\r", ""))
        assert tick and ("pit=off" not in (getattr(vm.args, "machine", None) or "")) == ("NOT COUNTING" not in tick[0]), vm.log[-3000:]
    hardware_report_check(vm)
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
    # Issue 171: no fixed count of tasks or applications. 40 applications at once, more than the former 8 and than the
    # former 32 tasks of the whole system (the task table grows a chunk); afterwards the kernel heap is back where it was.
    many = 40
    for pid in range(6, 6 + many):
        require(vm.command("run memtest hold 0 &"), f"PID={pid} NAME=memtest")
    assert len(task_rows(vm)) == many
    tasks = int(re.search(r"TASKS=(\d+)/", vm.command("free"))[1])
    assert tasks > 32 + many // 2, tasks
    # STAT pages (171-KRN-0007): every task's record, and from the 40th on only the rest.
    whole = re.search(r"STAT TASKS VERSION=\d+ COUNT=(\d+) TOTAL=(\d+) FROM=0", vm.command("stat tasks", raw=True))
    page = vm.command("stat tasks from 40", raw=True)
    rest = re.search(r"STAT TASKS VERSION=\d+ COUNT=(\d+) TOTAL=(\d+) FROM=40", page)
    assert whole and rest and int(whole[2]) == tasks and int(rest[1]) == tasks - 40 == len(re.findall(r"^\d+ PARENT=", page, re.M)), (whole, page)
    for pid in range(6, 6 + many):
        vm.command(f"kill {pid}")
    # And more than the former 127 endpoints: 40 programs that each create the 4 loader allows.
    for pid in range(6 + many, 6 + 2 * many):
        require(vm.command("run memtest endpoints &"), f"PID={pid} NAME=memtest")
    for pid in range(6 + many, 6 + 2 * many):
        require(vm.program_logs(pid, "ENDPOINTS"), "[MEMTEST] ENDPOINTS 4")
    endpoints = int(re.search(r"ENDPOINTS=(\d+)/", vm.command("free"))[1])
    assert endpoints > 127 + many, endpoints
    for pid in range(6 + many, 6 + 2 * many):
        vm.command(f"kill {pid}")
    many *= 2
    assert heap_used(vm) == baseline, "growing the tables and reusing slots leaked resources"
    # Repeated creation/freeing must retain the same empty-system heap baseline.
    first = 6 + many
    for pid in range(first, first + 10):
        require(vm.command("run app &"), f"PID={pid} NAME=app BACKGROUND")
        vm.command(f"kill {pid}")
    assert heap_used(vm) == baseline
    vm.send("boot\n")
    vm.expect(f"PID={first + 10} NAME=app FOREGROUND")
    vm.hmp("sendkey esc")
    vm.serial()
    assert task_rows(vm) == {}
    vm.keys("run clock &\n")
    vm.serial()
    clock = first + 11
    assert clock in task_rows(vm), "PS/2 Shift+7 must produce a background launch"
    vm.keys(f"fg {clock}\n")
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
    assert clock in task_rows(vm), "PS/2 fg/Ctrl+Z must preserve the task"
    vm.keys(f"kill {clock}\n")
    vm.serial()
    assert task_rows(vm) == {}
    # Idle: the CPU sleeps in HLT (one sample may catch it handling a tick, so a few are taken). QEMU shows no WFI
    # state for aarch64: there the emulator's own processor time over a second shows the CPU mostly asleep.
    if vm.arch == "aarch64":
        used = idle_cpu_seconds(vm)
        assert used < IDLE_LIMIT, f"idle QEMU used at least {used:.2f} s of processor time a second: a CPU does not wait in WFI"
    else:
        for _ in range(10):
            registers = vm.hmp("info registers")
            if "HLT=1" in registers:
                break
            time.sleep(.02)
        require(registers, "HLT=1")
    cpus = vm.hmp("info cpus")
    assert len(re.findall(r"CPU #\d", cpus)) == vm.cpus, cpus
    vm.serial()
    # Every CPU the machine has is online in the guest (issue 171: no cap of 8).
    online = vm.command("cpus")
    assert len(re.findall(r"ONLINE=true", online)) == vm.cpus, online
    assert heap_used(vm) == baseline
    print(f"PASS: instances, concurrent progress, fg, Ctrl+Z/UART+PS2, Esc, kill, logs, invalid input, reuse, heap, HLT, {vm.cpus} CPUs", flush=True)
    if vm.cpus > 8:
        monitors_every_cpu(vm)
        clocks_on_many_cpus(vm)
    pool_covers_free_ram(vm)
    memory = gibibytes(getattr(vm.args, "memory", None))
    # Filling a larger machine takes a thousand programs and more: the default machine runs out after about 80. It runs
    # with every CPU count, 16 too (171-APP-0008).
    if memory <= 1:
        applications_until_memory_ends(vm)
    if memory > 4:
        ram_above_4g(vm)
        # 171-KRN-0033: a large machine runs out at its frame pool too, not at the arena. memtest holds all but about
        # 400 MiB (40-odd programs), then clocks start until the pool ends, as on the default machine.
        holders = hold_frames(vm, frames_free(vm), 400)
        applications_until_memory_ends(vm, others=len(holders))
        for pid in holders:
            vm.command(f"kill {pid}")
    # 211-KRN-0044: init's first line names the branch and commit built (02_build.sh). Last: dmesg takes a PID.
    log = vm.command("dmesg -s init", raw=True)
    built = re.search(r"\[INIT\] BUILD: BRANCH (\S+), COMMIT ([0-9a-f]{12}(?:\+CHANGES)?)", log)
    assert built and built[1] != "UNKNOWN", log[-1500:]


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
    consoles_check(vm)


def consoles_check(vm):
    """Virtual consoles (issue 155): Ctrl+Alt+F1…F4 reach the shell whatever program has the keyboard and never that
    program; each console has its own text, history and programs; a program in a console not shown keeps running, and
    what it printed is there when the console is shown again; ps names the console of each program."""
    def typed(text):
        for char in text:
            vm.hmp(f"sendkey {dict(zip(' -', ('spc', 'minus'))).get(char, char)}")
            time.sleep(.06)

    def logged(text, start, timeout=8):
        # `text` in the log since `start` (what came while the harness used the monitor counts too).
        deadline = time.monotonic() + timeout
        while text not in vm.log[start:].replace("\r", ""):
            assert time.monotonic() < deadline, (text, vm.log[start:][-2000:])
            time.sleep(.02)
            vm.collect()
        vm.serial(enter=False)

    def switch(n, note):
        start = len(vm.log)
        vm.hmp(f"sendkey ctrl-alt-f{n}")
        logged(f"[SHELL] {note}\n", start)

    def last_prompt(screen):
        return next(row.replace("▁", " ").rstrip() for row in reversed(screen) if row.startswith("MIND> "))  # without the cursor

    vm.send("run keys\n")
    vm.expect("[KEYS] READY")
    start = len(vm.log)
    switch(2, "CONSOLE 2 SHOWN")
    typed("wintest show w 2")
    vm.hmp("sendkey ret")
    time.sleep(.5)
    switch(1, "CONSOLE 1 SHOWN (ITS PROGRAM HAS THE KEYBOARD)")
    at = len(vm.log)
    vm.hmp("sendkey q")
    logged("[KEYS] code=Char mods=- char=q", at)
    got = re.findall(r"\[KEYS\] (code=[^\r\n]*)", vm.log[start:])
    assert not any("code=F(" in line or "char=w" in line for line in got), ("keys saw the switch keys or console 2's typing", got)
    vm.send_bytes(b"\x1b")
    require(vm.expect("EXITED. SHELL RESUMED."), "[KEYS] DONE")
    time.sleep(.1); vm.collect(); vm.output = ""
    # ps: wintest runs in console 2 while console 1 is shown.
    rows = vm.command("ps")
    assert re.search(r"^\d+ wintest .* CONSOLE=2$", rows, re.M), rows
    time.sleep(4)
    switch(2, "CONSOLE 2 SHOWN")
    time.sleep(.3)
    screen = screen_text(vm)
    vm.serial(enter=False)
    text = "\n".join(screen)
    assert screen[0].rstrip().endswith("CONSOLE 2") and "CONSOLE 2. CTRL+ALT+F1" in text, screen[:3]
    assert "[WINTEST] w DONE AFTER" in text and "NAME=keys" not in text, text
    assert last_prompt(screen) == "MIND>", screen
    # Each console its own history: Up recalls console 2's command here, console 1's on the serial line.
    vm.hmp("sendkey up")
    time.sleep(.3)
    screen = screen_text(vm)
    vm.serial(enter=False)
    assert last_prompt(screen) == "MIND> wintest show w 2", screen[-6:]
    vm.hmp("sendkey esc")
    switch(1, "CONSOLE 1 SHOWN")
    vm.send_bytes(b"\x1b[A")
    vm.expect("MIND> ps")
    vm.send_bytes(b"\x1b")
    time.sleep(.5); vm.collect(); vm.output = ""
    assert task_rows(vm) == {}, task_rows(vm)
    print("PASS: virtual consoles: Ctrl+Alt+F2 opens console 2 while keys has the keyboard, keys never sees the switch keys; "
          "wintest keeps running there while console 1 is shown and its output is there after; each console its own history; ps names the console", flush=True)


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
    vm.command("time")
    # History: Up twice is "cpus".
    keys(b"\x1b[A\x1b[A\r", "CPU=0 APIC=")
    # Esc clears a typed line.
    vm.send_bytes(b"garbage\x1b")
    time.sleep(.2)
    keys(b"heap\r", "HEAP: USED=")
    # Tab completion of a program name after RUN, and of a command.
    keys(b"run dzen-c\t&\r", "PID=1 NAME=dzen-clock BACKGROUND")
    keys(f"kil\t{BASE + 1}\r".encode(), "KILLED PID=1")  # raw bytes: the harness does not translate the PID
    # Several matches are listed under the line: the shell's commands, then the programs (clock is one, issue u007);
    # the prompt drawn again ends the listing.
    vm.send_bytes(b"c\t")
    listing = vm.expect("MIND> ", after="cpus")
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
    # Scrollback: after enough output the banner is off the screen; Shift+PgUp brings it back. Three helps push it off
    # every screen and keep it within the 400 lines kept, with the boot log's last lines below it (211-PRT-0004).
    for _ in range(3):
        vm.command("help")
    assert not any(canon("MIND CORE v1.6") in row for row in screen_text(vm))
    for _ in range(12):
        vm.hmp("sendkey shift-pgup")
    screen = screen_text(vm)
    assert any(canon("MIND CORE v1.6") in row for row in screen), screen
    vm.hmp("sendkey shift-pgdn")
    vm.serial()
    print("PASS: shell line editing (Home/End/Left/Delete), history, Esc, Tab completion, Cyrillic input and display, PS/2 history, scrollback", flush=True)
    msh_check(vm)
    line_faces_check(vm)
    # pins (issue u015): no pin controller on QEMU (issue 206), so no gpio client to show; it says so and ends with 1.
    require(vm.command("pins"), "pins: no client of the gpio service")
    require(vm.command("pins 3"), "pins: no client of the gpio service")
    # pinmap (issue u017) says it on its screen; Esc ends it.
    vm.send("pinmap\n")
    vm.expect("NO PIN CONTROLLER", after="[PINMAP] READY")  # the whole line: it may arrive in pieces (000-APP-0004)
    time.sleep(.3)
    screen = screen_text(vm)
    vm.serial(enter=False)
    assert any("No pin controller" in row for row in screen), screen
    vm.send_bytes(b"\x1b")
    vm.expect("SHELL RESUMED.")
    time.sleep(.2); vm.collect(); vm.output = ""
    print("PASS: pins and pinmap without a pin controller say so", flush=True)
    # date set (211-APP-0042): the shell's rtc client may set the clock (rtc.wit 1.2); date reads it back; a date that
    # does not exist or is past 2099 is refused; the clock goes back to the host's time for what follows.
    import datetime
    require(vm.command("date set 2031-05-17 08:30:15"), "CLOCK SET")
    require(vm.command("date"), "DATE: 2031-05-17 08:30:1")
    for wrong in ("2031-02-30 08:30", "2100-01-01 00:00", "2031-05-17"):
        require(vm.command(f"date set {wrong}"), "USAGE: DATE SET YYYY-MM-DD HH:MM[:SS]")
    require(vm.command("date sit"), "THIS COMMAND TAKES NO ARGUMENTS")
    require(vm.command(f"date set {datetime.datetime.now():%Y-%m-%d %H:%M:%S}"), "CLOCK SET")
    print("PASS: date set: the clock set by the shell and read back; impossible dates refused", flush=True)


def line_faces_check(vm):
    """`clock --line` and `dzen-clock --line` (issue u016): console programs in the shell, their line written again
    with \\r every second: one line on the screen, the updates on the serial line; Esc stops them."""
    for command, line, seconds in (("clock --line", r"\d\d:\d\d:\d\d  \d{4}-\d\d-\d\d", 3.2), ("dzen-clock --line", r"\d\d:\d\d:\d\d  \S\S \S \S\S", 2.2)):
        start = len(vm.log)
        vm.send(command + "\n")
        vm.expect(f"NAME={command.split()[0]} FOREGROUND")
        time.sleep(seconds)
        screen = screen_text(vm)
        vm.serial(enter=False)
        vm.collect()
        shown = [row.strip() for row in screen if re.fullmatch(line, row.strip())]
        assert len(shown) == 1, (command, shown, screen[-12:])
        updates = re.findall("\r" + line, vm.log[start:])
        assert len(updates) >= int(seconds) - 1, (command, updates)
        vm.send_bytes(b"\x1b")
        vm.expect("MIND> ")
    assert task_rows(vm) == {}, task_rows(vm)
    print("PASS: clock --line and dzen-clock --line run as console programs: one line on the screen, written again every second; Esc stops them", flush=True)
    rtc_load_check(vm)


def rtc_load_check(vm):
    """000-APP-0012: a clock reads the RTC service about once a minute and counts the seconds between, so four clocks
    in the background add a few messages to its endpoint, not 10 a second each."""
    rtc = vm.services()["rtc"]
    def rtc_messages():
        return sum(int(m) for m, server in re.findall(r"^EP=\d+ .*MESSAGES=(\d+) .*SERVER=(\d+)", vm.command("endpoints", raw=True), re.M) if int(server) == rtc)
    start = rtc_messages()
    pids = [int(re.search(r"PID=(\d+) NAME=clock BACKGROUND", vm.command("run clock &"))[1]) for _ in range(4)]
    time.sleep(3)  # each clock waits out one change of the RTC's second when it starts
    before = rtc_messages()
    time.sleep(5)
    added = rtc_messages() - before
    for pid in pids:
        vm.command(f"kill {pid}")
    # Starting, each clock read the RTC until its second changed: the count sees the clocks' calls.
    assert before - start >= 4, f"{before - start} messages to the RTC service while four clocks started"
    assert added < 20, f"{added} messages to the RTC service in 5 s with 4 clocks (10 a second each was 200)"
    print(f"PASS: four clocks: {before - start} messages to the RTC service as they started, {added} in the next 5 s (they read it once a minute)", flush=True)


# msh scripts on the shell suite's disk (issue 094).
MSH_SCRIPTS = {
    "sum.msh": """#!msh
requires: files
# The files on ram:, their sizes summed into ram:summary.txt; a missing file handled.
let total = 0
let names = []
for f in files("ram:")? {
    if f.dir { continue }
    total = total + f.size
    names = push(names, f.name)
}
write ram:summary.txt "{len(names)} files, {total} bytes: {join(names, " ")}"
cat ram:missing.txt or { print("handled: {error}") }
print("sum done: {total}")
""",
    "services.msh": """#!msh
# Is vfs_server running? From ps's text, and from ps() records.
let out = capture("ps")?
if !contains(out, " vfs_server ") { fail("vfs_server is not running") }
let running = []
for task in ps()? {
    if task.service { running = push(running, task.name) }
}
if !contains(running, "logd") { fail("logd is not running") }
print("vfs_server runs; {len(running)} services; ps printed {len(lines(out))} lines")
""",
    "net0.msh": """#!msh
ping 10.0.2.2
print("not reached")
""",
    "net1.msh": """#!msh
requires: network
ping 10.0.2.2 or { print("ping failed: {error}") }
print("network allowed")
""",
    "check.msh": """#!msh
fn twice(x) { return x * 2 }
print(twice(nope(1)))
""",
    "broken.msh": """#!msh
let x = 1
let = 2
""",
}


def msh_check(vm):
    """msh (issue 094): a script that sums the files on ram: and handles a missing one; one that checks services from
    ps's captured text and ps() records; requires: network refused and granted; statements at the prompt; msh --check;
    a script outside the boot disk asks first; Ctrl+Z stops an endless loop."""
    require(vm.command("write ram:a.txt hello"), "WROTE 6 BYTES")
    require(vm.command("write ram:b.txt hi there"), "WROTE 9 BYTES")
    out = vm.command("msh data/sum.msh")
    require(out, "handled: CAT: NOT FOUND")
    require(out, "sum done: 15")
    require(vm.command("cat ram:summary.txt"), "2 files, 15 bytes: a.txt b.txt")
    require(vm.command('print(glob("ram:*.TXT")?)'), '["ram:a.txt", "ram:b.txt", "ram:summary.txt"]')
    require(vm.command("msh data/services.msh"), "vfs_server runs; ")
    # The network: refused without requires:, granted with it (whatever ping then finds).
    out = vm.command("msh data/net0.msh")
    require(out, "SCRIPT FAILED: ping needs `requires: network` in the script (LINE 2)")
    assert "not reached" not in out, out
    out = vm.command("data/net1.msh")
    require(out, "network allowed")
    assert "requires: network" not in out, out
    # A script's name runs it; statements typed at the prompt keep their variables and functions.
    vm.command("let n = 2 + 3")
    vm.command("fn sq(x) { return x * x }")
    require(vm.command('print("n = {n}, n squared = {sq(n)}")'), "n = 5, n squared = 25")
    require(vm.command('msh -c "let s = 0; for i in range(1, 5) { s = s + i }; print(s)"'), "10")
    # How a program ended (issue 166): grep that finds nothing exits with 1, a failure the script handles.
    require(vm.command('msh -c "grep zzz ram:a.txt or { print(error) }"'), "grep exited with 1")
    out = vm.command('msh -c "grep hel ram:a.txt; print(40 + 2)"')
    require(out, "hello")
    require(out, "42")
    require(vm.command("print(undefined_name)"), "SCRIPT FAILED: undefined_name is not defined (LINE 1)")
    # msh --check: names that do not exist, parse errors with line and column.
    require(vm.command("msh --check data/check.msh"), "MSH: data/check.msh: line 3, column 13: no function nope")
    require(vm.command("msh --check data/sum.msh"), "MSH: data/sum.msh: OK, REQUIRES: files")
    require(vm.command("msh data/broken.msh"), "MSH: data/broken.msh: line 3, column 5: expected a name")
    # A script outside the boot disk asks before it uses what it declares.
    require(vm.command("write ram:w.msh requires: files"), "WROTE 16 BYTES")
    vm.send("msh ram:w.msh\n")
    vm.expect("SCRIPT ram:w.msh REQUIRES files. ALLOW? (Y/N)")
    vm.send_bytes(b"n")
    require(vm.expect("MIND> "), "MSH: ram:w.msh: NOT RUN")
    # Ctrl+Z (0x1A on the serial line) stops a script in an endless loop.
    vm.send('msh -c "let i = 0; while true { i = i + 1 }"\n')
    time.sleep(1)
    vm.send_bytes(b"\x1a")
    require(vm.expect("MIND> "), "SCRIPT STOPPED: stopped (LINE 1)")
    require(vm.command("help msh"), "- msh <file> [args], msh -c")
    print("PASS: msh: a script sums the files on ram: and handles a missing one, another checks services through ps; requires: network refused and granted; "
          "statements at the prompt keep variables and functions; --check and parse errors with line and column; a script from ram: asks first; Ctrl+Z stops a loop", flush=True)


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
    fbank_check(vm)
    dictate_check(vm)
    speak_check(vm)
    monitors_check(vm)


def fbank_check(vm):
    """250: dictate computes the dictation models' features in the system (built with SSE2 on x86, in soft float on
    aarch64, where programs may not use FP/SIMD yet) as kaldi-native-fbank does on the host (tests/fbank_reference.txt)."""
    reference = [list(map(float, line.split())) for line in (ROOT / "tests/fbank_reference.txt").read_text().splitlines() if not line.startswith("#")]
    vm.send("dictate --features fbank.wav\n")
    out = vm.expect("MIND> ", timeout=60, after="dictate --features fbank.wav\n")
    head = re.search(r"FBANK SAMPLES=(\d+) FRAMES=(\d+) IN (\d+) US", out)
    assert head and int(head[1]) == len(FBANK_SIGNAL) and int(head[2]) == len(reference), out[-2000:]
    rows = dict(re.findall(r"^F(\d+) ([-\d. ]+)$", out.replace("\r", ""), re.M))
    worst = 0.0
    for frame, want in enumerate(reference):
        got = list(map(float, rows[str(frame)].split()))
        assert len(got) == 80, (frame, len(got))
        worst = max(worst, max(abs(a - b) for a, b in zip(got, want)))
    assert worst < 2e-3, worst
    print(f"PASS: dictate's features in the system equal kaldi-native-fbank's ({head[2]} frames, largest difference {worst:.1e}, "
          f"{int(head[3]) / 1000:.1f} ms)", flush=True)


# The toy transducer's text of the test signal (tests/nn_host.rs, TOY_TEXT).
TOY_TEXT = "нет нет дом нет нет дом нет нет дом нет нет дом нет нет"


def dictate_check(vm):
    """250: dictate runs the whole chain in the system (a network file read and checked, features, encoder, greedy
    search) and gives the host's text, with tests/dictate_toy.bin (scripts/voice_dictate/toy.py) in place of the 71 MB
    model; and refuses a damaged file."""
    def run(command):
        vm.send(command + "\n")
        return vm.expect("MIND> ", timeout=60, after=command + "\n")
    out = run("dictate --model toy.bin fbank.wav")
    simd = re.search(r"DICTATE: MODEL 9604 BYTES, READ IN \d+ MS, CHECKED IN \d+ MS; SIMD (AVX2|NONE)", out)
    assert simd and f"TEXT: {TOY_TEXT}" in out and "331 MS OF SPEECH" in out, out[-2000:]
    require(run("dictate --model toy-damaged.bin fbank.wav"), "dictate: toy-damaged.bin: Format(\"checksum\")")
    print(f"PASS: dictate in the system: the toy transducer's text is the host's (SIMD {simd[1]}); a damaged file is refused", flush=True)


def speak_dictionary():
    """252: a small MINDDIC1 dictionary (scripts/voice_tts/dictionary.py) over a made-up phoneme table, the text speak
    reads, and the ids vosk-tts's algorithm gives for it on the host."""
    sys.path.insert(0, str(ROOT / "scripts/voice_tts"))
    sys.dont_write_bytecode = True  # nothing written into scripts/
    import dictionary as voice_dictionary
    names = ["_", "^", "$", " ", "!", ",", ".", "-"] + [v + s for v in "aoueiy" for s in "01"] + \
        [c + soft for c in "bvgdzklmnprstfh" for soft in ("", "j")] + ["zh", "c", "ch", "sh", "sch", "j"]
    table = {n: [i] for i, n in enumerate(names)}
    rules = voice_dictionary.convert
    best = {"говорит": rules("говор+ит"), "разум": rules("р+азум"), "корабля": rules("корабл+я"), "ёлка": rules("+ёлка"),
            "мкс": ["e0", "m", "k", "a0", "e1", "s"]}  # an abbreviation, kept with its phonemes
    text = "Говорит разум корабля — ёлка, МКС! Неизвестное слово."
    data = voice_dictionary.build(table, best)[0]
    return data, text, voice_dictionary.ids(text, table, best)


def speak_check(vm):
    """252: speak's Russian front end in the system (dictionary, rules, punctuation) gives the host's ids, and a
    damaged dictionary is refused."""
    want = speak_dictionary()[2]
    def run(command):
        vm.send(command + "\n")
        return vm.expect("MIND> ", timeout=60, after=command + "\n")
    out = run("speak --dictionary speak.dic --ids --file speak.txt")
    assert f"IDS: {' '.join(map(str, want))}" in out, (want, out[-2000:])
    require(run("speak --dictionary speak-damaged.dic --ids --file speak.txt"), "speak: speak-damaged.dic: checksum")
    print(f"PASS: speak in the system: the Russian front end gives the host's {len(want)} phoneme ids; a damaged dictionary is refused", flush=True)


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


def logged(vm, start, text, timeout=8):
    # The log from `start` once a whole line with `text` came: what a program printed while the harness was in the QEMU
    # monitor (where `expect` and `serial` drop the output) counts too.
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        vm.collect()
        got = to_ordinal(ANSI.sub("", vm.log[start:]).replace("\r", ""))
        if text in got and "\n" in got[got.index(text):]:
            return got
        time.sleep(.02)
    raise AssertionError(f"Timeout waiting for {text!r}: {vm.log[start:][-3000:]}")


def mouse_moves(dx, dy):
    # QEMU monitor commands that move the PS/2 mouse by (dx, dy): QEMU queues few packets per command, so small steps.
    moves = []
    while dx or dy:
        step = (max(-100, min(100, dx)), max(-100, min(100, dy)))
        moves.append(f"mouse_move {step[0]} {step[1]}")
        dx, dy = dx - step[0], dy - step[1]
    return moves


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
    require(vm.command(f"budget {clock} 20 100"), f"BUDGET PID={clock} 20 MS PER 100 MS")  # top shows it (000-APP-0034)
    # top: the task table agrees with ps; details, sorting, filter and tree.
    vm.send("top\n")
    vm.expect("[TOP] READY")
    time.sleep(1.5)
    screen = screen_text(vm)
    vm.serial()  # Enter: the details window of the selected task
    assert canon(f"Tasks {tasks}:") in screen[1], screen[1]
    assert canon("load average") in screen[0], screen[0]
    assert table_row(screen, r"PID +PPID NAME +STATE") and table_row(screen, r" clock +") and table_row(screen, r" top +"), screen
    assert table_row(screen, r"PPID NAME .* BUDGET") and re.search(r" 20/100\*? *$", table_row(screen, r" clock +")), table_row(screen, r" clock +")
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
    # init's details list every capability it holds (171-APP-0007; 51 here, pages past 64 are host-tested).
    vm.send_bytes(b"\x1b[H")
    vm.expect("SELECTED=1 ")
    vm.send("\n")
    listed = int(re.search(r"LISTED=(\d+)", status_line(vm, "DETAILS=1 "))[1])
    vm.send_bytes(b"\x1b")
    vm.expect("DETAILS=0")
    vm.send("S")
    assert "ROWS=2" in status_line(vm, "HIDE=1"), "only clock and top are applications"
    vm.send("t")
    vm.expect("TREE=1")
    vm.send("q")
    require(vm.expect("EXITED. SHELL RESUMED."), "[TOP] DONE")
    time.sleep(.1); vm.collect(); vm.output = ""
    total = int(re.search(r"STAT CAPS VERSION=\d+ COUNT=\d+ TOTAL=(\d+)", vm.command("stat caps 1", raw=True))[1])
    assert listed == total, (listed, total)
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
    assert table_row(screen, re.escape(canon("Kernel arena 64.0M: used"))) and table_row(screen, canon(f"Tasks {tasks}, endpoints ")), screen  # counts alone (171-APP-0006)
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
    assert table_row(screen, r" loader +2/65279 "), screen
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
    for name in [f"CPU{cpu} " for cpu in range(vm.cpus)] + ["interrupts ", "syscalls ", "IPC messages ", "context switches ", "kernel arena ", f"tasks  {tasks}  max "]:  # scaled to its own maximum (171-APP-0006)
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
    assert table_row(screen, r"\d+ endpoints, .* messages, senders wait in order"), screen
    for service in ("vfs_server", "loader", "sysmon", "logd", "rtc"):
        assert table_row(screen, fr"^ +\d+ +{service} \(PID \d+\) +\d+ +\d+ "), (service, screen)
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
    # A program started from the panel takes fm's place in front (issue 160): top gets the keys, and Esc brings fm back
    # without the shell being told (fm stayed in front for it).
    # Down through the root's entries: about 80 now, the boot manifest and its signature among them (350-UPD-0002).
    for _ in range(120):
        if "CURRENT=top.elf " in keys(b"\x1b[B", "[FM] LEFT=/ FULL"):
            break
    else:
        raise AssertionError("top.elf not reached")
    mark = len(vm.log)
    vm.send_bytes(b"\r")
    vm.expect("[TOP] READY")
    vm.send_bytes(b"N")
    vm.expect("[TOP] SORT=PID")
    time.sleep(.3)
    screen = screen_text(vm)
    vm.serial(enter=False)
    assert table_row(screen, r"PID +PPID NAME +STATE") and table_row(screen, r" fm +"), screen
    vm.send_bytes(b"\x1b")
    vm.expect("[TOP] DONE")
    time.sleep(.3)
    screen = screen_text(vm)
    vm.serial(enter=False)
    started = table_row(screen, re.escape(canon("Started top.elf (PID")))
    assert started and canon("fm comes back when it ends") in started, screen
    assert table_row(screen, canon("1Help")), screen
    vm.collect(); vm.output = ""
    poke("[FM] LEFT=/ FULL")  # fm has the keys again
    assert "SHELL RESUMED" not in vm.log[mark:], vm.log[mark:]
    # Ctrl+Z from a program fm started still goes to the shell; FG brings fm back, the program stays in the background.
    for _ in range(300):  # every entry of the boot disk's root at most, however many programs it holds
        if "CURRENT=clock.elf " in keys(b"\x1b[A", "[FM] LEFT=/ FULL"):
            break
    else:
        raise AssertionError("clock.elf not reached")
    vm.send_bytes(b"\r")
    time.sleep(.5)
    vm.send_bytes(b"\x1a")
    pid = int(re.search(r"PID=(\d+) BACKGROUND\. SHELL RESUMED\.", vm.expect("BACKGROUND. SHELL RESUMED."))[1])
    time.sleep(.2)
    rows = task_rows(vm)
    assert rows[pid][0] == "clock", rows
    fm = next(p for p, row in rows.items() if row[0] == "fm")
    vm.send(f"fg {fm}\n")
    time.sleep(.3)
    vm.collect(); vm.output = ""
    poke("[FM] LEFT=/ FULL")
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
    # The mouse (issue u001): it starts in the middle of the screen (cell 80, 25). A click puts the cursor on an entry,
    # a second click soon after opens it, the wheel moves the cursor; the cell under the mouse is shown inverted.
    def mouse(*commands, text=None):
        start = len(vm.log)
        for command in commands:
            vm.hmp(command)
            time.sleep(.05)
        vm.serial(enter=False)
        return text and logged(vm, start, text)
    clicked = mouse(*mouse_moves((10 - 80) * 8, (3 - 25) * 16), "mouse_button 1", "mouse_button 0", text="CURRENT=EFI ")
    require(clicked, "[FM] POINTER 10,3 BUTTONS=1 WHEEL=0")
    assert "LEFT=/ FULL" in clicked, clicked
    mouse("mouse_button 1", "mouse_button 0", "mouse_button 1", "mouse_button 0", text="LEFT=/EFI FULL")
    require(mouse("mouse_move 0 0 -1", text="CURRENT=BOOT "), "[FM] POINTER 10,3 BUTTONS=0 WHEEL=1")
    mouse(*mouse_moves(0, 30 * 16))  # moves are not logged
    time.sleep(.3)
    _, size, _, pixels = vm.screenshot().split(b"\n", 3)
    vm.serial(enter=False)
    width = int(size.split()[0])
    corner = lambda x, y: pixels[(y * 16 * width + x * 8) * 3:(y * 16 * width + x * 8) * 3 + 3]
    assert corner(10, 33) != corner(12, 33) and corner(9, 33) == corner(12, 33), (corner(10, 33), corner(12, 33))
    vm.send_bytes(b"\x1b[21~")
    require(vm.expect("EXITED. SHELL RESUMED."), "[FM] DONE")
    time.sleep(.1); vm.collect(); vm.output = ""
    assert pid in task_rows(vm), task_rows(vm)
    time.sleep(1.2)
    require(vm.command(f"logs {pid}"), "[CLOCK] ")
    require(vm.command(f"kill {pid}"), "KILLED")
    for _ in range(20):
        if heap_used(vm) == baseline:
            break
        time.sleep(.1)
    assert heap_used(vm) == baseline
    print("PASS: fm: two panels with sizes and dates, the built-in viewer, EFI/BOOT and back, a program started from the panel in front and fm back after it, "
          "Ctrl+Z from it to the shell, the command line, Ctrl+O, Ctrl+F1 and Ctrl+P, "
          "the mouse (click, double click, wheel, the cell under it inverted)", flush=True)
    vfs_check(vm)
    console_check(vm)


def console_check(vm):
    """console (issues u004, 162): a terminal for programs on a screen of its own; a console program it starts prints
    into it through the endpoint it lends, and keys typed there start more."""
    vm.send("console uptime\n")
    require(vm.expect("[CONSOLE] ENDED uptime"), "[CONSOLE] RUN uptime PID")
    vm.send_bytes("grep -i -c строка docs/notes.txt\r".encode())
    vm.expect("[CONSOLE] ENDED grep")
    vm.send_bytes(b"nosuch\r")
    vm.expect("[CONSOLE] nosuch: no such program")
    time.sleep(.3)
    screen = screen_text(vm)
    vm.serial(enter=False)
    assert screen[1] == canon("> uptime").ljust(len(screen[1])) and screen[2].startswith(canon("up 0:")), screen[:4]
    assert any(row.startswith(canon("> grep -i -c строка docs/notes.txt")) for row in screen), screen
    assert any(row.rstrip() == "300" for row in screen), screen
    assert any(row.startswith(canon("nosuch: no such program")) for row in screen), screen
    assert screen[-1].startswith(canon("> ")), screen[-1]
    # console's own commands (issue u006): ps, ls; a shell command is named as such, ping without a network says why.
    vm.send_bytes(b"clear\r")
    for line in (b"ps", b"ls docs", b"kill 1", b"ping ya.ru"):
        vm.send_bytes(line + b"\r")
        time.sleep(.6)
    screen = screen_text(vm)
    vm.serial(enter=False)
    assert any(re.match(r" +1 init +RECV", row) for row in screen), screen
    assert any(row.startswith(canon("notes.txt ")) for row in screen), screen
    assert any(row.startswith(canon("kill: a command of the shell")) for row in screen), screen
    assert any(row.startswith(canon("ping: ")) for row in screen), screen
    vm.send_bytes(b"exit\r")
    require(vm.expect("EXITED. SHELL RESUMED."), "[CONSOLE] DONE")
    time.sleep(.1); vm.collect(); vm.output = ""
    assert task_rows(vm) == {}, task_rows(vm)
    # Only the task in front hands over the focus (issue 160): console in the background starts top in the background
    # too, and the shell keeps the keys.
    console = int(re.search(r"PID=(\d+) NAME=console BACKGROUND", vm.command("run console top &"))[1])
    got = ""
    for _ in range(25):  # a program in the background prints into its log, not to the serial line
        got += vm.command(f"logs {console}")
        if "[CONSOLE] RUN top PID" in got:
            break
        time.sleep(.2)
    assert "[CONSOLE] RUN top PID" in got and "ITS OWN SCREEN IN FRONT" not in got, got
    rows = task_rows(vm)
    top = next(p for p, row in rows.items() if row[0] == "top")
    assert rows[top][2] == "BG" and rows[console][2] == "BG", rows
    require(vm.command(f"kill {top}"), "KILLED")
    require(vm.command(f"kill {console}"), "KILLED")
    assert task_rows(vm) == {}, task_rows(vm)
    print("PASS: console: uptime and grep print into its terminal, an unknown program is named, ps and ls of its own, shell commands named, exit returns to the shell; "
          "in the background it cannot give its program the focus", flush=True)


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
    # Without a budget the loop takes all the time its CPU has that other tasks leave, over 3 s: its run time against
    # the CPU's busy plus idle time less the other tasks' run time there. The share of the host's wall clock is not
    # checked: under TCG a slow or busy host makes the compositor's copy of the screen and the other tasks take more
    # of it, and the loop's share of it ranged 0.52-0.73 (requests-KRN.md, 000-KRN-0026).
    def sample():
        tasks, cpus = vm.command("stat tasks", raw=True), vm.command("stat cpus", raw=True)
        rows = {int(m[1]): (m[2], int(m[3]), int(m[4])) for m in re.finditer(r"^(\d+) PARENT=\d+ (\S+) WAIT=\S+ CPU=(\d+) RUN_MS=(\d+)", tasks, re.M)}
        return rows, {int(m[1]): int(m[2]) + int(m[3]) for m in re.finditer(r"^CPU (\d+) APIC=\d+ ONLINE=1 BUSY_MS=(\d+) IDLE_MS=(\d+)", cpus, re.M)}
    (tasks0, cpus0), start = sample(), time.monotonic()
    time.sleep(3)
    tasks1, cpus1 = sample()
    loop = next(pid for pid, row in tasks1.items() if row[0] == "app2")
    cpu = tasks1[loop][1]
    others = sum(row[2] - tasks0[pid][2] for pid, row in tasks1.items() if pid != loop and pid in tasks0 and row[1] == cpu == tasks0[pid][1])
    left = (cpus1[cpu] - cpus0[cpu]) - others
    share = (tasks1[loop][2] - tasks0[loop][2]) / left
    wall = (tasks1[loop][2] - tasks0[loop][2]) / ((time.monotonic() - start) * 1000)
    assert share > 0.9, ("no budget: the loop takes the time other tasks leave on its CPU", share, left, others, wall)
    require(vm.command("kill 1"), "KILLED PID=1")
    vm.command("kill 2")
    assert heap_used(vm) == baseline
    print(f"PASS: timer preemption of a non-yielding {'register' if vm.arch == 'aarch64' else 'SIMD'} loop; responsive shell, clocks and kill; top shows the loop at ~100 % of its CPU; CPU budget per period; "
          f"without one the loop takes {share:.2f} of the time other tasks leave its CPU ({wall:.2f} of the wall clock)", flush=True)


def avx_expected(vm, fixture=None):
    """With a CPU model that has AVX (--cpu-model max) every CPU saves AVX state and the busy fixture uses AVX."""
    cpus = vm.command("cpus")
    model = getattr(vm.args, "cpu_model", None)
    if vm.arch == "aarch64":  # soft-float: no FP state is saved
        assert len(re.findall(r"FPU=NONE", cpus)) == vm.cpus, cpus
    elif model == "max":
        assert len(re.findall(r"FPU=XSAVE\+AVX", cpus)) == vm.cpus, cpus
        assert fixture is None or "CALLS, AVX" in fixture, fixture
    elif model is None:
        assert len(re.findall(r"FPU=FXSAVE", cpus)) == vm.cpus, cpus
    if vm.arch != "aarch64":
        # 174-KRN-0037: the area holds the enabled components; the padded test kernel's is AMX's size.
        state = re.search(r"VECTOR STATE: (XSAVE|FXSAVE), XCR0 0x([0-9A-F]+), (\d+) BYTES A TASK", vm.log)
        assert state, vm.log[-3000:]
        assert state[1] == ("XSAVE" if model == "max" else "FXSAVE") or model not in (None, "max"), state[0]
        padded = "xsave-pad" in (getattr(vm.args, "kernel", None) or "")
        assert (int(state[3]) >= 11 * 1024) == padded, (state[0], padded)


def smp_suite(vm):
    baseline = heap_used(vm)
    cpus = vm.command("cpus")
    assert len(re.findall(r"ONLINE=true", cpus)) == vm.cpus, cpus
    avx_expected(vm)
    protection_check(vm)
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
    # The rtc restarted while the applications ran took the next free slot of the task table, which may be past its
    # first chunk of 32 (the killed one is reaped after its successor starts): restarted again now, it takes a slot in
    # the first chunk, and the table gives the second back. Then the kernel heap is exactly where it was.
    rtc = vm.services()["rtc"]
    require(vm.command(f"kill {rtc}", raw=True), "KILLED PID=")
    require(vm.service_logs("init", "rtc RESTARTED"), "rtc RESTARTED")
    assert heap_used(vm) == baseline
    if vm.arch == "aarch64":
        # No WFI state in QEMU's monitor: with every CPU idle the emulator uses little processor time.
        vm.serial()
        used = idle_cpu_seconds(vm)
        assert used < IDLE_LIMIT, f"idle QEMU used at least {used:.2f} s of processor time a second: a CPU does not wait in WFI"
    else:
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
    kept, idle = ("register", "WFI") if vm.arch == "aarch64" else ("SIMD", "HLT")
    print(f"PASS: {vm.cpus} online CPUs, concurrent pinned tasks, {kept} preservation, supervisor reserve under load, remote kill, all CPUs {idle}", flush=True)


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
    for key, children, done in [("q", 5, "QUEUE ORDER OK"), ("j", 1, "LATE REPLY OK"), ("z", 2, "MOVE OK"), ("b", 1, "REVOKE PENDING OK"), ("i", 1, "BADGE OK")]:
        pid, faults = family(key, children, done)
        assert not any(f"FAULT PID={pid + n} " in faults for n in range(children + 1)), (key, faults)
    # The child keeps reading a lease when the parent revokes it: its next access faults (CAP_REVOKE waits for its CPU).
    pid, faults = family("x", 1, "LEASE REVOKED")
    assert re.search(fr"FAULT PID={pid + 1} CPU=\d+ VECTOR=14 ", faults), faults
    assert int(task_rows(vm)[1][-1]) > int(before[-1])
    # 000-KRN-0039: SGDT from a program faults (#GP) where UMIP is on.
    names = protection_check(vm)
    vm.send("run app2\n")
    pid = int(re.search(r"STARTED PID=(\d+) NAME=app2", vm.expect("RING3 IOPL0 READY"))[1])
    vm.send("U\n")
    output = vm.expect(f"PID={pid} EXITED. SHELL RESUMED.")
    time.sleep(.1); vm.collect(); vm.output = ""
    if "UMIP" in names:
        faults = vm.command("faults")
        assert re.search(fr"FAULT PID={pid} CPU=\d+ VECTOR=13 ", faults), faults
    else:
        require(output, "SGDT ALLOWED")
    vm.command("kill 1")
    assert heap_used(vm) == baseline, "fault teardown leaked task/page-table resources"
    require(vm.command("run app &"), "NAME=app BACKGROUND")
    print(f"PASS: CPL3/IOPL0; kernel read/write, RX code, NX stack, CLI/I/O, UD2, guard/bad stack; syscall pointers; capability checks and endpoint badges; fault containment and reclaim; kernel protection {' '.join(names)}", flush=True)


def protection_check(vm):
    """000-KRN-0039: what the kernel turned on against reaching programs' pages, as the CPU model offers it. The
    protection-test kernel reads and jumps into a program's page once: SMAP and SMEP must stop both."""
    line = re.search(r"MIND CORE KERNEL: PROTECTION: ([A-Z ]+?)\r?\n", vm.log)
    assert line, vm.log[-2000:]
    names, model = line[1].split(), getattr(vm.args, "cpu_model", None)
    if vm.arch == "aarch64":
        assert names == ["PXN", "PAN"], line[0]  # -cpu max has PAN (ARMv8.1)
    elif model in (None, "max"):
        assert names == (["SMEP", "SMAP", "UMIP"] if model == "max" else ["NONE"]), line[0]
    if "protection" in (getattr(vm.args, "kernel", None) or ""):
        require(vm.log, "PROTECTION TEST: READ OF A PROGRAM'S PAGE FAULTED, FETCH FROM IT FAULTED")
    return names


def memory_suite(vm):
    baseline, frames = heap_used(vm), frames_free(vm)
    # Task memory comes from the frame pool (issue 150): memtest holds all but about 40 MiB of what applications may take, so spawns run out.
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
    memory_charged_to_spawner(vm)
    recovery_reserve(vm)
    print("PASS: out-of-memory rollback, surviving tasks and later successful launch", flush=True)


def recovery_reserve(vm):
    """Issue 169: applications that take all the heap they can leave init's recovery reserve in the frame pool; a
    service killed meanwhile is restarted and serves."""
    frames = frames_free(vm)
    holders = []
    for _ in range(8):
        output = vm.command("run memtest fill &")
        started = re.search(r"STARTED PID=(\d+)", output)
        if not started:
            require(output, "OUT OF MEMORY")  # not even an image and a stack fit above the reserve
            break
        holders.append(int(started[1]))
        filled = int(re.search(r"FILLED (\d+) KiB", vm.program_logs(holders[-1], "FILLED"))[1])
        if filled < 160 * 1024:
            break
    else:
        raise AssertionError("applications did not run out of memory")
    free = frames_free(vm)
    # Above the reserve stays less than the smallest block memtest asks for (and an image); services may dip into it.
    assert RECOVERY_RESERVE - 1024 * 1024 <= free < RECOVERY_RESERVE + 1024 * 1024, (free, RECOVERY_RESERVE)
    rtc = vm.services()["rtc"]
    vm.command(f"kill {rtc}", raw=True)
    for _ in range(40):
        if vm.services().get("rtc", rtc) != rtc:
            break
        time.sleep(.25)
    else:
        raise AssertionError("rtc was not restarted while applications held all they could")
    assert re.search(r"\d{4}-\d\d-\d\d", vm.command("date")), "the restarted rtc does not serve"
    for pid in holders:
        vm.command(f"kill {pid}")
    for _ in range(40):
        if frames_free(vm) >= frames - 1024 * 1024:
            break
        time.sleep(.25)
    else:
        raise AssertionError("the holders' frames were not returned")
    print(f"PASS: applications holding all they could ({len(holders)} memtest) left {free} bytes of the frame pool (reserve {RECOVERY_RESERVE}); rtc restarted and served meanwhile", flush=True)


def task_memory(vm, pid, raw=False):
    # (image + stack + screen + kernel structures, memory used by the task and its descendants) from `stat <pid>`.
    details = vm.command(f"stat {pid}", raw=raw)
    sizes = re.search(r"IMAGE=(\d+) STACK=(\d+) SCREEN=(\d+) .* KERNEL=(\d+) ", details)
    used = re.search(r"MEMORY=(\d+)/\d+", details)
    assert sizes and used, details
    return sum(map(int, sizes.groups())), int(used[1])


def memory_charged_to_spawner(vm):
    """Issue 168: a program's image, stack and screen are charged to its spawner (loader) and leave its account at exit;
    its kernel structures too (171-KRN-0032)."""
    loader = vm.services()["loader"]
    def loader_used():
        previous = None
        for _ in range(20):
            used = task_memory(vm, loader, raw=True)[1]
            if used == previous:
                return used
            previous = used
            time.sleep(.1)
        raise AssertionError("loader's memory account did not settle")
    before = loader_used()
    output = vm.command("run clock &")
    started = re.search(r"PID=(\d+) NAME=clock BACKGROUND", output)
    assert started, output
    pid = int(started[1])
    for _ in range(20):
        fixed, used = task_memory(vm, pid)
        after = loader_used()
        if after - before == fixed + used and task_memory(vm, pid) == (fixed, used):
            break
        time.sleep(.2)
    else:
        raise AssertionError(f"loader's account grew by {after - before}, the program holds {fixed} + {used}")
    assert fixed > 64 * 1024, fixed  # the stack alone is 64 KiB, the screen more
    vm.command(f"kill {pid}")
    for _ in range(20):
        if loader_used() == before:
            break
        time.sleep(.2)
    else:
        raise AssertionError(f"loader's account {loader_used()} did not return to {before}")
    print(f"PASS: a program's image, stack, screen and kernel structures ({fixed} bytes) charged to loader and returned at exit", flush=True)


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
    # The frame pool (issue 150) is larger than four quotas: memtest holds all but about 40 MiB of what applications may take.
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
    # The text faces (issue 089): the indicators as colored cells with the same keys, the clock in large digits.
    vm.send("dzen-clock --text\n")
    require(vm.expect("[DZEN-CLOCK] 19:3"), "[DZEN-CLOCK] STARTED (TEXT)")
    time.sleep(.5)

    def look():
        screen = screen_text(vm)
        _, size, _, pixels = vm.screenshot().split(b"\n", 3)
        vm.serial()  # the Enter of leaving the monitor: neither face uses it
        width, height = map(int, size.split())
        def count(color, x0, x1, y0, y1):
            return sum(pixels[(y * width + x) * 3:(y * width + x) * 3 + 3] == color for y in range(y0, y1, 2) for x in range(x0, x1, 2))
        return screen, width, height, count

    screen, width, height, count = look()
    assert canon("DZEN CLOCK") in screen[0] and canon("D: DIGITS") in screen[-1] and canon("19:3") in screen[-2], (screen[0], screen[-2:])
    assert count(b"\xff\xff\x00", width // 2, width, 0, height // 2) > 300, "the hour's yellow disc at the top right"
    assert count(b"\x00\xff\xff", width // 4, 3 * width // 4, height // 4, 3 * height // 4) > 300, "the cyan center"
    assert count(b"\x60\x60\x60", 0, width, 0, height) > 300, "white corners"
    vm.send("c\n")
    vm.expect("ORBIT SIMPLE")
    time.sleep(.3)
    screen = look()[0]
    # The dot reads back as ● or as its inverse ◘: a cell of two colours is matched with either as the foreground.
    assert sum(row.count(canon("●")) + row.count(canon("◘")) for row in screen) == 1 and sum(row.count(canon("·")) for row in screen) > 10, screen
    vm.send("d\n")
    vm.expect("DIGITS OFF")
    vm.send("h\n")
    vm.expect("TEXT OFF")
    time.sleep(.3)
    screen = look()[0]
    assert not screen[0].strip() and not screen[-1].strip() and not screen[-2].strip(), (screen[0], screen[-2:])
    # On its own screen T switches to the pixel face and back (issue u007).
    vm.send("t\n")
    require(vm.expect("[DZEN-CLOCK] STARTED. D: DIGITS"), "[DZEN-CLOCK] PIXEL FACE")
    vm.send("t\n")
    require(vm.expect("[DZEN-CLOCK] STARTED (TEXT)"), "[DZEN-CLOCK] TEXT FACE")
    time.sleep(.3)
    screen = look()[0]
    assert canon("T: PIXEL FACE") in screen[-1], screen[-1]
    vm.send("\x1b")
    require(vm.expect("EXITED. SHELL RESUMED."), "[DZEN-CLOCK] RETURNING TO KERNEL.")
    time.sleep(.1); vm.collect(); vm.output = ""
    vm.send("clock --text\n")  # the program (issue u007: the shell's one-line command is `time`)
    vm.expect("[CLOCK] 19:3")
    time.sleep(.3)
    first = look()[0]
    assert any(canon("▀") in row or canon("▄") in row for row in first), first
    assert any(canon("Saturday 2026-09-19") in row for row in first) and canon("CLOCK (IPC RTC)") in first[0], first
    time.sleep(1.2)
    assert look()[0] != first, "the digits change every second"
    vm.send("\x1b")
    vm.expect("EXITED. SHELL RESUMED.")
    time.sleep(.1); vm.collect(); vm.output = ""
    assert heap_used(vm) == baseline
    assert "FAULT PID=" not in vm.command("faults")
    print("PASS: dzen-clock colors; small clockwise dot; darker C orbit; bottom-right start and 10s ticks; UART/PS2 C/P/D/H; clean title/hint toggle; mode switching and erasure; independent instances; fg/exit/reclaim; the text faces of dzen-clock (T switches faces) and clock", flush=True)


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
    size = (ROOT / IMAGE / "kernel.elf").stat().st_size
    require(output, f"READ kernel.elf {size}/{size} BYTES MAGIC=7F454C46")
    efi = (ROOT / IMAGE / BOOT_EFI).stat().st_size
    require(output, f"READ {BOOT_EFI} {efi}/{efi} BYTES MAGIC=4D5A")
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


def blockstore_check(vm):
    """300-KRN-0001 (requested by the storage track): init starts the block store over a RAM disk of its own (ramdisk#1)
    and gives the shell a client with the get, put and publish badges in SLOT_BLOCKSTORE (25)."""
    ready = "[BLOCKSTORE] READY BLOCKS=0 NAMES=0 SECTORS=1/16384 CORRUPT=0 DAMAGED=0"
    require(vm.service_logs("blockstore", ready), ready)
    real = vm.services()
    assert "ramdisk#1" in real, real
    caps = vm.command(f"stat caps {real['shell']}", raw=True)
    found = re.search(r"^SLOT=25 GEN=0 KIND=1 RIGHTS=6 SIZE=0 BADGE=7 EP=(\d+)", caps, re.M)
    servers = {int(ep): int(server) for ep, server in re.findall(r"^EP=(\d+) .*SERVER=(\d+)", vm.command("endpoints", raw=True), re.M)}
    assert found and servers.get(int(found[1])) == real["blockstore"], caps
    print("PASS: the block store runs over ramdisk#1 (empty, 8 MiB); the shell holds its client with get, put and publish", flush=True)


def cid_raw(data):
    """The CIDv1 text of raw content (multiformats: version 1, raw 0x55, sha2-256 0x12, 32 bytes), computed here
    independently of libmind's cid.rs."""
    import base64, hashlib
    return "b" + base64.b32encode(bytes([1, 0x55, 0x12, 0x20]) + hashlib.sha256(data).digest()).decode().lower().rstrip("=")


def store_suite(vm):
    """Main tasks 300-302 (storage track): the block store through the `blocks` tool and the shell's client (badge
    get, put and publish). An object of 4 MiB + 1 byte (the pattern of tests/dag_host.rs) gets the root an independent
    reference computed (tests/dag_host.rs, Python dag-cbor), and reads back equal; a file round trip; names change only
    from the version expected and only to complete roots; a full store refuses with full and keeps what it holds; a
    restarted store finds its blocks and names again on its medium."""
    def blocks(args, timeout=240):
        vm.send(f"blocks {args}\n")
        return vm.expect("MIND> ", timeout=timeout, after=f"blocks {args}\n")
    ready = "[BLOCKSTORE] READY BLOCKS=0 NAMES=0 SECTORS=1/16384 CORRUPT=0 DAMAGED=0"
    require(vm.service_logs("blockstore", ready), ready)
    require(blocks("stat"), "BLOCKS=0 NAMES=0 BYTES=0 SECTORS=1/16384 CORRUPT=0 DAMAGED=0 CAPACITY=4096")
    # Height 2: a root over two nodes of height 1 (256 chunks and 1 chunk).
    root = "bafyreiczboab4oohlzcsoyt6wuxoz3r5m2z5blyi5pj46ah6q2pyc3d5ai"
    require(blocks("pattern 4194305"), f"PUT {root} SIZE 4194305")
    require(blocks(f"check {root} pattern"), "CHECKED 4194305 BYTES = PATTERN")
    # A file: its CID is the raw CID of its bytes, and it comes back the same.
    require(vm.command("write ram:note.txt hello store"), "WROTE 12 BYTES")
    note = cid_raw(b"hello store\n")
    require(blocks("put ram:note.txt"), f"PUT {note} SIZE 12")
    require(blocks(f"get {note} ram:copy.txt"), "GOT 12 BYTES")
    require(vm.command("cat ram:copy.txt"), "hello store")
    # 300-STO-0004: blocksro asks only to read and holds the client badged get alone (300-KRN-0024). It reads; a put
    # and a publish are refused, and the store logs each with its badge.
    require(vm.command(f"blocksro get {note} ram:ro.txt"), "GOT 12 BYTES")
    out = vm.command("blocksro put ram:note.txt")
    require(out, "blocks: put:"); require(out, "Rights")
    require(vm.command(f"blocksro publish ro {note}"), "blocks: publish ro: Rights")
    log = vm.service_logs("blockstore", "REFUSED Publish")
    assert re.search(r"\[BLOCKSTORE\] REFUSED Put FOR PID \d+ \(BADGE 1\)", log) and re.search(r"\[BLOCKSTORE\] REFUSED Publish FOR PID \d+ \(BADGE 1\)", log), log
    require(blocks("resolve ro"), "blocks: resolve ro: NotFound")
    # Names: compare-and-swap on the version; only roots whose blocks are all stored.
    require(blocks(f"publish obj {root}"), "PUBLISHED obj VERSION 1")
    require(blocks(f"publish obj {note}"), "blocks: publish obj: Conflict")
    require(blocks(f"publish obj {note} 1"), "PUBLISHED obj VERSION 2")
    require(blocks(f"resolve obj"), f"obj VERSION 2 ROOT {note}")
    require(blocks(f"publish ghost {cid_raw(b'never stored')}"), "blocks: publish ghost: Incomplete")
    require(blocks("resolve ghost"), "blocks: resolve ghost: NotFound")
    require(blocks("resolve a:b"), "blocks: resolve a:b: Invalid")
    require(blocks(f"publish obj {root} 2"), "PUBLISHED obj VERSION 3")
    published = "[BLOCKSTORE] PUBLISHED obj VERSION 3 ROOT " + root
    require(vm.service_logs("blockstore", published), published)
    # The medium runs out: a defined refusal, and what is stored stays readable.
    filled = re.search(r"FILLED (\d+) BLOCKS, THEN Full \(Some\(Full\)\)", blocks("fill", timeout=600))
    assert filled and int(filled[1]) > 100, filled
    stat = re.search(r"BLOCKS=(\d+) NAMES=1 BYTES=\d+ SECTORS=(\d+)/16384 CORRUPT=0 DAMAGED=0", blocks("stat"))
    assert stat and int(stat[2]) > 16384 - 33, stat
    # A new instance mounts the same medium: every block verified again, the names' latest versions current; the object
    # is checked whole after each restart (a check takes over a minute under TCG on aarch64).
    require(vm.command("svc restart blockstore"), "blockstore restarted: PID")
    again = f"[BLOCKSTORE] READY BLOCKS={stat[1]} NAMES=1 SECTORS={stat[2]}/16384 CORRUPT=0 DAMAGED=0"
    require(vm.service_logs("blockstore", again), again)
    # 303-STO-0001: right after a mount every block's lease runs (60 s): no block goes; the earlier versions' name
    # records may. Before the long check below, which takes more than a lease under TCG on aarch64.
    first = blocks("collect")
    assert re.search(r"COLLECTED 0 BLOCKS [0-2] NAMES [0-2] SECTORS", first), first
    require(blocks("resolve obj"), f"obj VERSION 3 ROOT {root}")
    require(blocks(f"check {root} pattern"), "CHECKED 4194305 BYTES = PATTERN")
    print("PASS: block store: a 4 MiB object gets the reference root and reads back; a file round trip; a client that may only "
          "read is refused put and publish; names by compare-and-swap, only complete roots; a full medium refused; a restarted "
          "store finds blocks and names again", flush=True)
    # What no name retains goes once its lease (60 s after the last put or the mount) has ended: a file put and never
    # published goes; the file kept as obj's second version stays, retained by the name's history (303-STO-0003).
    require(vm.command("write ram:loose.txt never named"), "WROTE 12 BYTES")
    loose = cid_raw(b"never named\n")
    require(blocks("put ram:loose.txt"), f"PUT {loose} SIZE 12")
    time.sleep(61)
    # The medium is full of blocks whose leases have ended: new ones get room from the collection the first put starts.
    require(blocks("fill 120", timeout=600), "FILLED 120 BLOCKS")
    # The new blocks' leases run: an explicit collection right after frees no block (before the long check below).
    collected = blocks("collect")
    assert re.search(r"COLLECTED 0 BLOCKS \d+ NAMES \d+ SECTORS, \d+ FREE", collected), collected
    require(vm.service_logs("blockstore", "[BLOCKSTORE] COLLECTED 0 BLOCKS"), "[BLOCKSTORE] COLLECTED 0 BLOCKS")
    require(blocks(f"get {loose} ram:gone.txt"), "blocks: get: NotFound")
    require(blocks(f"get {note} ram:kept.txt"), "GOT 12 BYTES")
    print("PASS: block store collection: once leases end, a put that needs room frees what nothing retains and the room "
          "is written again; a name's earlier version stays", flush=True)
    # 303-STO-0002..0004: history, quotas, pins and removing a name, for the shell's owner (badge 7).
    history = blocks("history obj")
    for line in (f"obj VERSION 3 ROOT {root}", f"obj VERSION 2 ROOT {note}", f"obj VERSION 1 ROOT {root}"):
        require(history, line)
    require(blocks(f"publish copy {root}"), "PUBLISHED copy VERSION 1")
    quota = 16384 * 512 // 4 * 3
    require(blocks("usage"), f"RETAINED {4194305 + 12} OF {quota} BYTES, 2 NAMES, 0 PINS")
    # 2 200 000 more bytes would pass three quarters of the medium: refused for a name and for a pin.
    other = re.search(r"PUT (\S+) SIZE 2200000", blocks("pattern 2200000"))[1]
    require(blocks(f"publish big {other}"), "blocks: publish big: Quota")
    require(blocks(f"pin {other}"), "blocks: pin: Quota")
    # The file is retained by obj's history already: a pin of it costs nothing more.
    require(blocks(f"pin {note}"), f"PINNED {note} AS 1")
    require(blocks("pins"), f"PIN 1 ROOT {note} SIZE 12")
    require(blocks("usage"), f"RETAINED {4194305 + 12} OF {quota} BYTES, 2 NAMES, 1 PINS")
    # Removing a name: a version without a root; the name retains nothing more, and it comes back only from that version.
    require(blocks("unpublish obj 2"), "blocks: unpublish obj: Conflict")
    require(blocks("unpublish obj 3"), "REMOVED obj VERSION 4")
    require(blocks("resolve obj"), "blocks: resolve obj: NotFound")
    require(blocks("history obj"), "obj VERSION 4 REMOVED")
    require(blocks("usage"), f"RETAINED {4194305 + 12} OF {quota} BYTES, 1 NAMES, 1 PINS")
    require(blocks("unpin 1"), "UNPINNED 1")
    require(blocks("pins"), "0 PINS")
    require(vm.service_logs("blockstore", "[BLOCKSTORE] UNPINNED 1"), "[BLOCKSTORE] REMOVED obj VERSION 4")
    # What the collections left mounts again whole: the removal, the copy's version, no pin.
    require(vm.command("svc restart blockstore"), "blockstore restarted: PID")
    require(vm.service_logs("blockstore", "NAMES=2 SECTORS="), "CORRUPT=0 DAMAGED=0")
    require(blocks("history obj"), "obj VERSION 4 REMOVED")
    require(blocks("resolve copy"), f"copy VERSION 1 ROOT {root}")
    require(blocks("pins"), "0 PINS")
    require(blocks(f"check {root} pattern"), "CHECKED 4194305 BYTES = PATTERN")
    print("PASS: block store retention: a name keeps its versions, an owner its quota, a pin its object until unpinned; "
          "a removed name retains nothing and mounts again as removed", flush=True)
    # 304-STO-0007: several names at once, all or none, read at one point between commits.
    require(vm.command("write ram:left.txt left"), "WROTE 5 BYTES")
    left = cid_raw(b"left\n")
    require(blocks("put ram:left.txt"), f"PUT {left} SIZE 5")
    done = blocks(f"commit copy 1 {left} pair 0 {root}")
    for line in ("COMMITTED copy VERSION 2", "COMMITTED pair VERSION 1"):
        require(done, line)
    logged = f"[BLOCKSTORE] COMMITTED pair VERSION 1 ROOT {root}"
    require(vm.service_logs("blockstore", logged), logged)
    # A stale version, or a root never stored, beside a valid change: nothing changes.
    require(blocks(f"commit copy 1 {root} pair 1 {left}"), "blocks: commit: Conflict")
    require(blocks(f"commit copy 2 {root} ghost 0 {cid_raw(b'never stored')}"), "blocks: commit: Incomplete")
    snap = blocks("snapshot copy pair ghost obj")
    for line in (f"copy VERSION 2 ROOT {left}", f"pair VERSION 1 ROOT {root}", "ghost NONE", "obj VERSION 4 REMOVED"):
        require(snap, line)
    # A removal and a change in one commit, found whole by a new instance.
    done = blocks(f"commit pair 1 - copy 2 {root}")
    for line in ("COMMITTED pair VERSION 2 REMOVED", "COMMITTED copy VERSION 3"):
        require(done, line)
    require(vm.command("svc restart blockstore"), "blockstore restarted: PID")
    require(vm.service_logs("blockstore", "NAMES=3 SECTORS="), "CORRUPT=0 DAMAGED=0")
    snap = blocks("snapshot copy pair")
    for line in (f"copy VERSION 3 ROOT {root}", "pair VERSION 2 REMOVED"):
        require(snap, line)
    print("PASS: block store commits: two names change at once or not at all (a stale version, a missing root), a "
          "snapshot reads them at one point, a commit with a removal is found whole after a restart", flush=True)
    # 306-STO-0009: checkpoints. Each run of tally is an instance: it restores the last checkpoint, applies one request
    # and saves the next, its manifest and state under two names in one commit.
    def tally(args):
        vm.send(f"tally {args}\n")
        return vm.expect("MIND> ", timeout=240, after=f"tally {args}\n")
    require(tally("show"), "TALLY EPOCH 0 SEQUENCE 0")
    require(tally("add apples 3"), "SAVED EPOCH 1 SEQUENCE 1: apples = 3")
    require(tally("add pears"), "SAVED EPOCH 2 SEQUENCE 2: pears = 1")
    require(tally("add apples"), "SAVED EPOCH 3 SEQUENCE 3: apples = 4")
    snap = blocks("snapshot checkpoint/tally checkpoint/tally/state")
    for line in ("checkpoint/tally VERSION 3 ROOT", "checkpoint/tally/state VERSION 3 ROOT"):
        require(snap, line)
    # An instance restores epoch 3 and stalls; another saves epoch 4 meanwhile; the stale one's save is refused whole.
    held = re.search(r"PID=(\d+) NAME=tally BACKGROUND", vm.command("run tally hold 6 pears &"))
    assert held, "tally did not start in the background"
    require(vm.program_logs(held[1], "HOLDING EPOCH"), "HOLDING EPOCH 3")
    require(tally("add apples"), "SAVED EPOCH 4 SEQUENCE 4: apples = 5")
    require(vm.program_logs(held[1], "FENCED"), "tally: FENCED: epoch 3 is no longer current; nothing saved")
    shown = tally("show")
    for line in ("TALLY EPOCH 4 SEQUENCE 4", "apples = 5", "pears = 1"):
        require(shown, line)
    # Effects: the intent is saved before the effect begins. An instance that ends between the effect and its record
    # leaves it pending; the next refuses new effects until someone who knows the outcome reconciles it (B.4).
    require(tally("effect fx.txt crash"), "EFFECT 1 BEGUN; ENDING BEFORE ITS OUTCOME IS RECORDED")
    require(vm.command("cat ram:fx.txt"), "done by tally")
    require(tally("show"), "EFFECT 1 write:ram:fx.txt PENDING: RECONCILE BEFORE TRYING AGAIN")
    require(tally("effect fy.txt"), "tally: effect refused: effect 1 is pending; reconcile it first")
    require(tally("reconcile 1 done"), "RECONCILED 1 SAVED EPOCH 6")
    require(tally("effect fy.txt"), "EFFECT 2 DONE SAVED EPOCH 8")
    print("PASS: checkpoints: tally restores and saves its counters across instances, a stale instance is fenced, an "
          "effect cut short comes back pending and is reconciled before another begins", flush=True)
    fm_store_check(vm)


def fm_store_check(vm):
    """300-APP-0038: fm shows the block store as store:; a file copied there is stored as an object and published under
    its name, F3 reads it back, F8 unpublishes it."""
    def keys(data, text):
        vm.send_bytes(data)
        return status_line(vm, text)
    text = b"from fm to the store\n"
    require(vm.command("write ram:fmnote.txt from fm to the store"), "WROTE 21 BYTES")
    vm.send("fm ram:\n")
    require(status_line(vm, "[FM] READY"), "LEFT=/ram: FULL")
    keys(b"\x1b[12;3~", "DIALOG=VOLUME")  # Alt+F2: A:, ram:, store: (no log: or models: here)
    keys(b"\x1b[B", "DIALOG=VOLUME")
    keys(b"\x1b[B", "DIALOG=VOLUME")
    keys(b"\r", "RIGHT=/store: BRIEF")

    def to(name):
        line = keys(b"\x1b[H", "CURRENT=")  # Home
        for _ in range(30):
            if f"CURRENT={name} " in line:
                return
            line = keys(b"\x1b[B", "CURRENT=")
        raise AssertionError(line)
    to("fmnote.txt")
    keys(b"\x1b[15~", "DIALOG=TARGET")  # F5: to the other panel, store:/
    vm.send_bytes(b"\r")
    require(status_line(vm, "JOB=NONE", whole=True), "RIGHT=/store: BRIEF")
    keys(b"\t", "ACTIVE=R")
    to("fmnote.txt")
    keys(b"\x1bOR", "VIEW=1")  # F3
    time.sleep(.3)
    screen = screen_text(vm)
    vm.serial()
    assert any(canon("from fm to the store") in row for row in screen), screen
    keys(b"\x1b", "VIEW=0")
    keys(b"\x1b[19~", "DIALOG=DELETE")  # F8
    vm.send_bytes(b"\r")
    status_line(vm, "JOB=NONE", whole=True)
    vm.send_bytes(b"\x1b[21~")  # F10
    require(vm.expect("EXITED. SHELL RESUMED."), "[FM] DONE")
    time.sleep(.1); vm.collect(); vm.output = ""
    log = vm.service_logs("blockstore", "REMOVED fmnote.txt")
    require(log, f"PUBLISHED fmnote.txt VERSION 1 ROOT {cid_raw(text)}")
    require(log, "REMOVED fmnote.txt VERSION")
    print("PASS: fm's store: panel: a file copied there is published as an object, viewed, and unpublished with F8", flush=True)


def free_port():
    with socket.socket() as probe:
        probe.bind(("127.0.0.1", 0))
        return probe.getsockname()[1]


# The gdbstub port of the storefaults suite's VM (300-STO-0005).
GDB_PORT = free_port()


def guest_ram(vm):
    """The guest's RAM: its first physical address (x86 from 0, QEMU virt from 1 GiB) and size."""
    return (0x4000_0000 if vm.args.arch == "aarch64" else 0), int(gibibytes(getattr(vm.args, "memory", None)) * (1 << 30))


def poke(vm, needle, offset, port):
    """Fault injection from the host (300-STO-0005): in every copy of `needle` in the guest's RAM, the byte `offset`
    bytes after its start gets one bit flipped. The RAM is read with QMP pmemsave; the bytes are written through
    QEMU's gdbstub in physical-memory mode, which stops the guest for the write. Returns how many copies changed."""
    base, size = guest_ram(vm)
    with tempfile.TemporaryDirectory() as temp:
        dump = Path(temp) / "ram"
        vm.hmp(f'pmemsave {base} {size} "{dump}"')
        data = dump.read_bytes()
    places, at = [], data.find(needle)
    while at >= 0:
        places.append(at)
        at = data.find(needle, at + 1)
    with socket.create_connection(("127.0.0.1", port), timeout=10) as gdb:
        pending = b""
        def packet(text):
            nonlocal pending
            body = text.encode()
            gdb.sendall(b"$" + body + b"#%02x" % (sum(body) % 256))
            while True:
                start = pending.find(b"$")
                end = pending.find(b"#", start)
                if start >= 0 and end >= 0 and len(pending) >= end + 3:
                    reply, pending = pending[start + 1:end], pending[end + 3:]
                    gdb.sendall(b"+")
                    return reply.decode()
                pending += gdb.recv(4096)
        assert packet("Qqemu.PhyMemMode:1") == "OK"
        for at in places:
            byte = data[at + offset] ^ 0x10
            assert packet(f"M{base + at + offset:x},1:{byte:02x}") == "OK", "gdbstub refused the write"
        packet("D")
    return len(places)


def record_header(chunk):
    """A raw block's record header on the store's medium, up to its CID (layout 2, blockstore/src/store.rs)."""
    import hashlib
    cid = bytes([1, 0x55, 0x12, 0x20]) + hashlib.sha256(chunk).digest()
    return b"MIND-BLK" + (2).to_bytes(2, "little") + bytes(2) + len(chunk).to_bytes(4, "little") + cid


def store_faults_suite(vm):
    """300-STO-0005: damage on the block store's own medium, injected from the host into the RAM disk's bytes in guest
    memory (no program can reach that medium). A flipped byte in a chunk is refused when read and when mounting; a
    collection refuses while a name's object lacks it; a put of the same bytes repairs it and a collection frees the
    damaged copy. A damaged name record is reported, and the version before it stands. A damaged record header loses
    that record alone: its sectors are counted damaged, and the records after it are found."""
    def blocks(args, timeout=240):
        vm.send(f"blocks {args}\n")
        return vm.expect("MIND> ", timeout=timeout, after=f"blocks {args}\n")
    def restart():
        require(vm.command("svc restart blockstore"), "blockstore restarted: PID")
        return vm.service_logs("blockstore", "[BLOCKSTORE] READY")
    data = bytes((i * 31 + 7) % 251 for i in range(100000))
    chunk = lambda k: data[k * 16384:(k + 1) * 16384]
    root = "bafyreiaatjv3tf4ncvx6eeupemzvs5lz5zepknmtfeqlxdcibwwc5ae4ru"  # the reference's root of the pattern's first 100 000 bytes
    require(vm.service_logs("blockstore", "[BLOCKSTORE] READY"), "[BLOCKSTORE] READY")
    require(blocks("pattern 100000"), f"PUT {root} SIZE 100000")
    require(blocks(f"publish obj {root}"), "PUBLISHED obj VERSION 1")
    # A byte of the third chunk, 100 bytes into its data: refused when read, and the name's object is no longer whole.
    assert poke(vm, record_header(chunk(2)), 84 + 100, GDB_PORT) >= 1
    require(blocks(f"check {root} pattern"), "blocks: check: Corrupt (the store: Corrupt)")
    require(vm.service_logs("blockstore", "[BLOCKSTORE] CORRUPT"), f"[BLOCKSTORE] CORRUPT {cid_raw(chunk(2))}")
    require(blocks("stat"), "CORRUPT=1 DAMAGED=0")
    require(blocks("collect"), "blocks: collect: Incomplete")
    # A put of the same bytes stores the chunk again elsewhere; a collection then frees the damaged copy.
    require(blocks("pattern 100000"), f"PUT {root} SIZE 100000")
    require(blocks(f"check {root} pattern"), "CHECKED 100000 BYTES = PATTERN")
    assert re.search(r"COLLECTED [1-9]\d* BLOCKS", blocks("collect"))
    require(blocks("stat"), "CORRUPT=0 DAMAGED=0")
    # A byte of the fifth chunk, found when a new instance mounts the medium.
    assert poke(vm, record_header(chunk(4)), 84 + 100, GDB_PORT) >= 1
    require(restart(), "CORRUPT=1 DAMAGED=0")
    require(blocks(f"check {root} pattern"), "blocks: check: NotFound")
    require(blocks("pattern 100000"), f"PUT {root} SIZE 100000")
    require(blocks(f"check {root} pattern"), "CHECKED 100000 BYTES = PATTERN")
    # A damaged name record: the second version's. A new instance counts its sector damaged; the first version stands.
    require(vm.command("write ram:v2.txt second"), "WROTE 7 BYTES")
    second = cid_raw(b"second\n")
    require(blocks("put ram:v2.txt"), f"PUT {second} SIZE 7")
    require(blocks(f"publish obj {second} 1"), "PUBLISHED obj VERSION 2")
    import base64
    second_bytes = base64.b32decode(second[1:].upper() + "=" * (-len(second[1:]) % 8))
    name_record = b"MIND-REF" + (2).to_bytes(2, "little") + (3).to_bytes(2, "little") + (2).to_bytes(8, "little") + second_bytes
    assert poke(vm, name_record, 100, GDB_PORT) >= 1
    require(restart(), "DAMAGED=1")
    require(blocks("resolve obj"), f"obj VERSION 1 ROOT {root}")
    require(blocks(f"check {root} pattern"), "CHECKED 100000 BYTES = PATTERN")
    # A damaged header, in the second chunk's CID: that record is lost whole, its 33 sectors counted damaged with the
    # name record's one; the chunks after it are found. A collection first frees the fifth chunk's corrupt copy.
    assert re.search(r"COLLECTED [1-9]\d* BLOCKS", blocks("collect"))
    assert poke(vm, record_header(chunk(1)), 30, GDB_PORT) >= 1
    require(restart(), f"CORRUPT=0 DAMAGED={1 + (84 + 16384 + 511) // 512}")
    require(blocks(f"check {root} pattern"), "blocks: check: NotFound")
    require(blocks("pattern 100000"), f"PUT {root} SIZE 100000")
    require(blocks(f"check {root} pattern"), "CHECKED 100000 BYTES = PATTERN")
    # A damaged commit (304-STO-0007): a byte of its first entry; a new instance applies none of it, and counts its
    # header and both entries damaged.
    for path, text in (("ram:c1.txt", "first change"), ("ram:c2.txt", "second change")):
        require(vm.command(f"write {path} {text}"), f"WROTE {len(text) + 1} BYTES")
        require(blocks(f"put {path}"), f"PUT {cid_raw(text.encode() + bytes([10]))} SIZE {len(text) + 1}")
    first, other = cid_raw(b"first change\n"), cid_raw(b"second change\n")
    done = blocks(f"commit obj 1 {first} aux 0 {other}")
    for line in ("COMMITTED obj VERSION 2", "COMMITTED aux VERSION 1"):
        require(done, line)
    txn = b"MIND-TXN" + (2).to_bytes(2, "little") + (2).to_bytes(2, "little") + bytes(4)
    assert poke(vm, txn, 512 + 100, GDB_PORT) >= 1
    require(restart(), f"CORRUPT=0 DAMAGED={34 + 3}")
    snap = blocks("snapshot obj aux")
    for line in (f"obj VERSION 1 ROOT {root}", "aux NONE"):
        require(snap, line)
    print("PASS: block store damage injected on its medium: a chunk refused when read and when mounting, a collection "
          "refused meanwhile, a put repairs it; a damaged name record reported, the version before it stands; a damaged "
          "header loses its record alone; a damaged commit changes no name", flush=True)
    # 305-STO-0008: a crash. init restarts the killed store within its budget, and the new instance mounts the medium.
    old = vm.services()["blockstore"]
    starts = int(re.search(r"^blockstore\s+\d+\s+(\d+)", vm.command("svc", raw=True), re.M)[1])
    vm.command(f"kill {old}", raw=True)
    for _ in range(40):
        if vm.services().get("blockstore", old) != old:
            break
        time.sleep(.25)
    else:
        raise AssertionError("init did not restart the block store")
    require(vm.service_logs("blockstore", "[BLOCKSTORE] READY"), "CORRUPT=0 DAMAGED=37")
    assert re.search(fr"^blockstore\s+\d+\s+{starts + 1}\s+running", vm.command("svc", raw=True), re.M)
    require(blocks("snapshot obj"), f"obj VERSION 1 ROOT {root}")
    # A medium it cannot mount (sector 0's digest damaged): the store stays up and answers every request with the
    # reason, and the system, started from the boot volume, goes on without it (MC-6.8, Appendix B.4).
    superblock = b"MIND-STO" + (2).to_bytes(2, "little") + (512).to_bytes(2, "little")
    assert poke(vm, superblock, 20, GDB_PORT) >= 1
    require(vm.command("svc restart blockstore"), "blockstore restarted: PID")
    require(vm.service_logs("blockstore", "NOT MOUNTED"), "[BLOCKSTORE] NOT MOUNTED: Foreign")
    require(blocks("stat"), "blocks: stat: Device")
    require(blocks("resolve obj"), "blocks: resolve obj: Device")
    require(blocks("pattern 10"), "blocks: pattern: Store (the store: Device)")
    started = re.search(r"PID=(\d+) NAME=clock BACKGROUND", vm.command("run clock &"))
    assert started, "a program from the boot volume does not start while the store is down"
    vm.command(f"kill {started[1]}")
    print("PASS: block store recovery: a killed store is restarted by init and mounts its medium again; a medium it "
          "cannot mount leaves it answering every request with the reason while programs start from the boot volume", flush=True)


def escrow_check(vm):
    """Issue 170: init keeps the privileges it grants in escrow and holds no process control: it can pass them to a
    service it starts but not use them. Services restarted from escrow get the privilege itself."""
    caps = vm.command("stat caps 1", raw=True)
    kinds = [int(k) for k in re.findall(r"^SLOT=\d+ GEN=\d+ KIND=(\d+) ", caps, re.M)]
    usable = {6: "input", 7: "display", 11: "platform", 12: "control", 14: "observe"}
    assert not {usable[k] for k in kinds if k in usable}, caps
    escrowed = {int(r) for r in re.findall(r"^SLOT=\d+ GEN=\d+ KIND=15 RIGHTS=(\d+) ", caps, re.M)}
    assert {6, 7, 9, 12, 14} <= escrowed, caps
    assert 13 in kinds and 9 in kinds, caps  # restart and its own spawn privilege
    # Restarted from escrow: sysmon reads statistics with the observe privilege, loader starts programs with spawn.
    # loader is ended from the shell and init restarts it (svc, which loader itself started, would race the shell's
    # call to loader that is still open while loader goes).
    old = vm.services()["sysmon"]
    require(vm.command("svc restart sysmon"), "sysmon restarted: PID")
    assert vm.services()["sysmon"] != old
    require(vm.service_logs("sysmon", "[SYSMON] READY: SAMPLES EVERY 100 MS"), "[SYSMON] READY: SAMPLES EVERY 100 MS")
    old = vm.services()["loader"]
    vm.command(f"kill {old}", raw=True)
    for _ in range(40):
        if vm.services().get("loader", old) != old:
            break
        time.sleep(.25)
    else:
        raise AssertionError("init did not restart loader")
    require(vm.service_logs("loader", "[LOADER] READY"), "[LOADER] READY")
    started = re.search(r"PID=(\d+) NAME=clock BACKGROUND", vm.command("run clock &"))
    assert started, "loader restarted from escrow does not start programs"
    vm.command(f"kill {started[1]}")
    require(vm.command("stat caps 1", raw=True), "KIND=15 RIGHTS=12 ")
    print("PASS: init holds no usable input, display, observe or process-control privilege (escrow only); sysmon and loader restarted from escrow work", flush=True)


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
    clocks = [re.search(r"MONOTONIC NS=(\d+) RESOLUTION NS=(\d+) TSC HZ=(\d+)", vm.command("time")) for _ in range(2)]
    assert re.search(r"TIME: \d\d:\d\d:\d\d UPTIME MS=\d+ ", vm.command("time")), "the time of day first"
    assert all(clocks), clocks
    (first, resolution, hz), (second, _, _) = [tuple(map(int, c.groups())) for c in clocks]
    assert second > first and 0 < resolution < 1_000_000 and hz > 1_000_000, (first, second, resolution, hz)
    # Observation (STAT): the task table agrees with ps, the memory summary with heap, and every CPU is online.
    tasks = len(re.findall(r"^\d+ [\w#-]+ ", vm.command("ps", raw=True), re.M))
    free = vm.command("free")
    assert f"TASKS={tasks}/65535 " in free and re.search(r"ENDPOINTS=\d+/65535 ", free), (tasks, free)
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
    assert int(re.search(r"STAT TASKS VERSION=2 COUNT=(\d+) TOTAL=\d+ FROM=0", tasks)[1]) == len(re.findall(r"^\d+ [\w#-]+ [A-Z_]+ (?:BG|FG) ", vm.command("ps", raw=True), re.M)), tasks
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
    # Quotas delegated at spawn: init holds the root quota and gives loader all of it but 256 kept for the services (issue 171).
    quotas = vm.command("quotas", raw=True)
    assert re.search(r"^\d+ loader 0/65279 0/65279$", quotas, re.M) and re.search(r"^1 init \d+/65535 \d+/65535$", quotas, re.M), quotas
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
    loader = vm.services()["loader"]
    def used(pid):
        return int(re.search(r"QUOTA TASKS=\d+/\d+ ENDPOINTS=\d+/\d+ MEMORY=(\d+)/", vm.command(f"stat {pid}", raw=True))[1])
    before = used(loader)
    require(vm.command("run hello &"), "PID=4 NAME=hello BACKGROUND")
    # 171-KRN-0032: the loader pays for what it started, the program's kernel structures too (its task, pages, page
    # tables and capability table, in the frame pool): its use grows by exactly that sum.
    for _ in range(20):
        parts = re.search(r"IMAGE=(\d+) STACK=(\d+) SCREEN=(\d+) HEAP=(\d+) .* RETAINED=(\d+) KERNEL=(\d+)", vm.command("stat 4"))
        grown = used(loader) - before
        if grown == sum(map(int, parts.groups())):
            break
        time.sleep(.2)
    assert grown == sum(map(int, parts.groups())) and int(parts[6]) > 0, (grown, parts.groups())
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
    require(details, "CAPS=5/4095")
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
    tasks = len(re.findall(r"^\d+ [\w#-]+ ", vm.command("ps", raw=True), re.M))
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
    console_flow_check(vm)
    lifecycle_check(vm)
    # help <name>: the program's text read from its file, the shell's own lines, or a service; nothing is started.
    output = vm.command("help fm")
    require(output, "Usage: fm [directory]")
    assert "STARTED" not in output, output
    output = vm.command("fm --help")  # a program with a screen: the shell shows its text instead of starting it
    require(output, "fm — file manager")
    assert "STARTED" not in output, output
    require(vm.command("help cat"), "- ls [path], cat <file>: files")
    # log: named in help, efivar described (211-APP-0013); the USB image test checks log: on a disk that has it.
    require(vm.command("help ls"), "log: the boot disk's log partition")
    require(vm.command("help write"), "on ram:, on log: and in data/")
    output = vm.command("efivar --help")
    require(output, "efivar — the firmware's boot variables")
    assert "ALLOW?" not in output, "--help asks the user for nothing"
    require(vm.command("help efivar"), "bootnext <hex>")
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
    blockstore_check(vm)
    escrow_check(vm)
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


def console_flow_check(vm):
    """000-KRN-0030: a console program that prints far more than the console's 4 KiB before the shell reads it loses
    nothing: the kernel takes what fits while the shell drains, and the program sends the rest again."""
    output = vm.command("cat lines.txt", raw=True)
    lines = re.findall(r"^LINE (\d{3}) [.]+$", output, re.M)
    assert lines == [f"{n:03}" for n in range(300)], (len(lines), output[:400], output[-400:])


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


def raw_fat_image(temp, replace=None, extra=None):
    """A raw disk image: an MBR with one EFI system partition (what OVMF boots from a fixed disk) holding a FAT16 file
    system of BLOCK_FS_MB with the built OS; `replace` maps file names to other files, `extra` paths to the bytes of
    more files. Returns (image, start, sectors)."""
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
    for name in [*(p.name for p in (ROOT / IMAGE).glob("*.elf")), BOOT_EFI]:
        shutil.copyfile(ROOT / IMAGE / name, files / name)
    for name, source in (replace or {}).items():
        shutil.copyfile(source, files / name)
    for name, data in (extra or {}).items():
        (files / name).parent.mkdir(parents=True, exist_ok=True)
        (files / name).write_bytes(data)
    sign_manifest.sign_volume(files)
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


# The video gateway's test pattern (libmind/src/video.rs, issue 158): eight bars moving left 4 pixels a frame, and the
# frame number in 32 cells of the bottom 16 rows.
VIDEO_BARS = (0xFFFFFF, 0xFFFF00, 0x00FFFF, 0x00FF00, 0xFF00FF, 0xFF0000, 0x0000FF, 0x000000)


def video_pattern(sequence, x, y, width, height):
    if y >= height - 16:
        return 0xFFFFFF if sequence >> (31 - x * 32 // width) & 1 else 0
    return VIDEO_BARS[(x + sequence * 4 % width) % width * 8 // width]


def camera_check(vm):
    """camera (issue 158) on the synthetic source: the shell asks before lending the camera, a refused program runs
    without it; a still and 3 s of video; the camera mark while the stream is open, gone after it."""
    def ask(command, answer):
        vm.send(command + "\n")
        vm.expect("CAMERA ASKS FOR THE CAMERA. ALLOW? (Y/N)")
        vm.send_bytes(answer)
    ask("camera -s data/cam.bmp", b"n")
    require(vm.expect("SHELL RESUMED."), "camera: no camera was granted")
    time.sleep(.2); vm.collect(); vm.output = ""
    ask("camera -s data/cam.bmp", b"y")
    still = re.search(r"\[CAMERA\] STILL data/cam.bmp: 320X240, FRAME (\d+), (\d+) BYTES", vm.expect("SHELL RESUMED.", timeout=30))
    assert still, vm.log[-2000:]
    time.sleep(.2); vm.collect(); vm.output = ""
    def stalled(*report):
        # Seen on CI and never locally (158-APP-0005): Ctrl+Z gives the shell back while the program runs on, and every task
        # of the camera's path says what it waits for. The check still fails.
        vm.send_bytes(b"\x1a")
        try:
            vm.expect("SHELL RESUMED.", timeout=20)
        except AssertionError as e:
            report += ("no shell after Ctrl+Z", str(e)[-500:])
        ps = vm.command("ps", raw=True)
        path = r"camera|video_gw|vfs_server|compositor|nvme|ata|ahci|ramdisk|rtc|logd|sysmon"
        waits = [vm.command(f"stat {pid}", raw=True) for pid in re.findall(rf"^(\d+) (?:{path}) ", ps, re.M)]
        raise AssertionError(report + (ps, waits, vm.service_logs("video_gw")))
    ask("camera -r 10 -t 3 data/cam.avi", b"y")
    vm.expect("[CAMERA] OPENED test pattern 320X240 AT 10/S")
    opened = time.monotonic()
    time.sleep(1)
    _, size, _, during = vm.screenshot().split(b"\n", 3)
    width = int(size.split()[0])
    mark = ((13 * width) + width - 48 - 6) * 3  # the camera mark's body, left of the capture dot
    if during[mark:mark + 3] != bytes((0x20, 0xC0, 0x40)):
        # Whether the mark comes late or blinks, or is not drawn at all.
        later = []
        for _ in range(4):
            time.sleep(.3)
            later.append((round(time.monotonic() - opened, 2), vm.screenshot().split(b"\n", 3)[3][mark:mark + 3].hex()))
        vm.serial(enter=False)
        # How much of the screen is drawn at all: black everywhere would mean nothing reached the framebuffer.
        pixels = len(during) // 3
        black = sum(1 for i in range(0, pixels * 3, 3 * 97) if during[i:i + 3] == b"\0\0\0") * 97 * 100 // pixels
        stalled("the camera mark while the stream is open", during[mark:mark + 3], f"{black}% of the screen black", "later:", later)
    vm.serial(enter=False)
    # 30 frames of camera time; a slow encoder (aarch64 under TCG) gets fewer pictures and repeats the last one.
    try:
        ended = vm.expect("SHELL RESUMED.", timeout=60)
    except AssertionError:
        stalled("the recording of 3 s did not end within a minute", round(time.monotonic() - opened, 1))
    video = re.search(r"\[CAMERA\] VIDEO data/cam.avi: 30 FRAMES \((\d+) PICTURES\) 320X240 AT 10/S, SEQUENCE (\d+)\.\.(\d+), TIMESTAMPS ON THE RATE, (\d+) BYTES", ended)
    assert video and int(video[3]) - int(video[2]) == 29 and int(video[1]) >= 3, (video and video.groups(), vm.log[-2000:])
    time.sleep(1.8)
    _, _, _, after = vm.screenshot().split(b"\n", 3)
    vm.serial(enter=False)
    assert after[mark:mark + 3] != bytes((0x20, 0xC0, 0x40)), "the mark goes out once no stream is open"
    logged_lines = vm.service_logs("video_gw", "CLOSED AFTER")
    require(logged_lines, "[VIDEO] SYNTHETIC SOURCE: video/synthetic on the boot disk")
    assert len(re.findall(r"\[VIDEO\] PID \d+ OPENED test pattern 320x240 AT 10/S", logged_lines)) == 2, logged_lines
    time.sleep(.2); vm.collect(); vm.output = ""
    print("PASS: camera: the shell asks first and a refused program runs without the camera; a still and 3 s of video of the test pattern; the camera mark while the stream is open", flush=True)
    return int(still[1]), (int(video[4]), int(video[1]))


def camera_files(mtype, frame, video):
    """The camera's files on the disk: the still is the test pattern's frame exactly, the video 30 Motion JPEG frames."""
    bmp = mtype("data/cam.bmp")
    assert bmp[:2] == b"BM" and struct.unpack_from("<iiHH", bmp, 18) == (320, 240, 1, 24), bmp[:54]
    stride = 320 * 3
    for y in range(240):
        row = bmp[54 + (239 - y) * stride:][:stride]
        for x in range(0, 320, 3):
            p = video_pattern(frame, x, y, 320, 240)
            assert row[x * 3:x * 3 + 3] == bytes((p & 0xFF, p >> 8 & 0xFF, p >> 16)), (x, y, row[x * 3:x * 3 + 3], hex(p))
    avi = mtype("data/cam.avi")
    size, pictures = video
    assert len(avi) == size and avi[:4] == b"RIFF" and avi[8:12] == b"AVI " and struct.unpack_from("<I", avi, 48)[0] == 30, avi[:64]
    chunks, at = [], 224
    while avi[at:at + 4] == b"00dc":
        length, = struct.unpack_from("<I", avi, at + 4)
        chunks.append(avi[at + 8:at + 8 + length])
        at += 8 + length + (length & 1)
    shown = [c for c in chunks if c]  # an empty chunk repeats the picture before it
    assert len(chunks) == 30 and chunks[0] and len(shown) == pictures and all(c[:2] == b"\xff\xd8" and c[-2:] == b"\xff\xd9" for c in shown) and len(set(shown)) == pictures, (len(chunks), len(shown))
    if shutil.which("ffprobe"):
        probe = Path(tempfile.gettempdir()) / "mind-core-cam.avi"
        probe.write_bytes(avi)
        info = subprocess.run(["ffprobe", "-v", "error", "-show_entries", "stream=codec_name,width,height,r_frame_rate,nb_frames", "-of", "csv=p=0", str(probe)],
                              capture_output=True, text=True).stdout.strip()
        assert info == "mjpeg,320,240,10/1,30", info
    print("PASS: camera files: the still is the test pattern's frame pixel for pixel, the video 30 Motion JPEG frames that ffprobe reads", flush=True)


def vfs_suite(args):
    """Writing a raw FAT disk through vfs_server: the shell changes files in data/, syncs, the host checks the image
    (fsck.fat, mtools), and after a reboot the files are there while the RAM disk is empty again."""
    if not raw_tools():
        print("SKIP: vfs suite needs mkfs.fat, fsck.fat and mtools", flush=True)
        return
    with tempfile.TemporaryDirectory(prefix="smoke-vfs-", dir=ROOT / IMAGE) as temp:
        # video/synthetic: the video gateway serves its test pattern (issue 158).
        image, start, fs_sectors = raw_fat_image(Path(temp), extra={"video/synthetic": b"the video gateway's test pattern stands in for a camera\n"})
        part = f"{image}@@{start * 512}"
        vm = VM(args, image.relative_to(ROOT).as_posix(), raw=True, snapshot=False)
        try:
            mounted = vm.service_logs("vfs_server", "AS RAM:")  # reading drains the log: both lines at once
            require(mounted, f"[VFS] MOUNTED FAT16 FROM {BOOT_DRIVE} AT LBA 2048 (DEVICE WRITABLE)")
            require(mounted, "[VFS] MOUNTED FAT16 FROM RAM AS RAM: (")
            for command, answer in [("mkdir data/sub", "OK"), ("write data/notes.txt line one", "WROTE 9 BYTES"), ("write data/sub/a.txt alpha", "WROTE 6 BYTES"),
                                    ("mv data/sub/a.txt data/b.txt", "OK"), ("rm data/sub", "OK"), ("write ram:temp.txt scratch", "WROTE 8 BYTES"), ("sync", "OK")]:
                require(vm.command(command), answer)
            # screenshot (issue 086): the screen in front (the shell's) as a BMP in data/, and under a free name on ram:.
            def slow(command):  # 3 MB written through the block driver take a while under emulation
                vm.send(command + "\n")
                return vm.expect("MIND> ", timeout=180, after=command + "\n")
            vm.command("clear")  # a screen with room: the output after the capture must not scroll it
            shot = re.search(r"SCREENSHOT data/screen.bmp: (\d+)x(\d+), (\d+) BYTES", slow("screenshot data/screen.bmp"))
            assert shot, vm.log[-2000:]
            screen = vm.screenshot()
            # The capture dot (issue 165) goes out 1.5 s after the last capture.
            time.sleep(1.6)
            after = vm.screenshot()
            vm.serial()
            require(slow("screenshot"), "SCREENSHOT ram:screen-001.bmp")
            require(slow("screenshot"), "SCREENSHOT ram:screen-002.bmp")
            require(vm.command("screenshot two words"), "USAGE: SCREENSHOT [FILE]")
            # record (issue 093): 3 s of the screen while clock is in front, into data/; the display shows the dot meanwhile.
            recorder = re.search(r"PID=(\d+) NAME=record BACKGROUND", vm.command("run record -t 3 data/rec.avi &"))[1]
            vm.send("clock\n")
            vm.expect("[CLOCK] ")
            time.sleep(1)
            _, size_, _, during = vm.screenshot().split(b"\n", 3)
            vm.serial(enter=False)
            dot_at = ((18 * int(size_.split()[0])) + int(size_.split()[0]) - 24) * 3
            assert during[dot_at:dot_at + 3] == bytes((0xE0, 0x20, 0x20)), "the capture dot while recording"
            time.sleep(3)  # clock stays in front for the rest of the recording
            vm.send("\x1b")
            vm.expect("SHELL RESUMED.")
            time.sleep(.2); vm.collect(); vm.output = ""
            for _ in range(60):
                if int(recorder) not in task_rows(vm):
                    break
                time.sleep(.5)
            else:
                raise AssertionError("record did not end")
            cam_still, cam_video = camera_check(vm)
            require(vm.command("sync"), "OK")
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
        rows = [bmp[54 + (height - 1 - y) * stride:][:width * 3] for y in range(30)]
        assert len({row[i:i + 3] for row in rows[:14] for i in range(0, width * 3, 3)} ) >= 2, "the line has text"
        # The display shows the screen the BMP holds; the capture dot (a red disc of radius 6 around (width - 24, 18)) may
        # still be over it (writing the BMP to the disk took a while) and is gone 1.6 s later.
        in_dot = lambda x, y: (x - (width - 24)) ** 2 + (y - 18) ** 2 <= 36
        later = after.split(b"\n", 3)[3]
        for y, row in enumerate(rows):
            # The command's own line whole (below it the shell has printed since); the dot's corner on the rows below.
            for x in range(width) if y < 14 else range(width - 31, width - 17):
                captured, shown, then = row[x * 3:x * 3 + 3][::-1], pixels[(y * width + x) * 3:(y * width + x) * 3 + 3], later[(y * width + x) * 3:(y * width + x) * 3 + 3]
                assert shown == captured or (in_dot(x, y) and shown == bytes((0xE0, 0x20, 0x20))), (x, y, shown, captured)
                assert then == captured, ("the dot is gone, the screen under it put back", x, y, then, captured)
        # The recording: an AVI of 30 Motion JPEG frames of the screen's size at 10 per second, a key frame each time the
        # screen changed (the clock's seconds) and an empty chunk (the frame again) otherwise, and its index.
        avi_file = mtype("data/rec.avi")
        assert avi_file[:4] == b"RIFF" and avi_file[8:12] == b"AVI " and struct.unpack_from("<I", avi_file, 4)[0] == len(avi_file) - 8, avi_file[:16]
        frames_, = struct.unpack_from("<I", avi_file, 48)
        assert struct.unpack_from("<II", avi_file, 64) == (width, height) and frames_ == 30 and struct.unpack_from("<I", avi_file, 132)[0] == 10, avi_file[:224]
        chunks, at = [], 224
        while avi_file[at:at + 4] == b"00dc":
            length, = struct.unpack_from("<I", avi_file, at + 4)
            chunks.append(avi_file[at + 8:at + 8 + length])
            at += 8 + length + (length & 1)
        assert len(chunks) == 30 and avi_file[at:at + 4] == b"idx1" and struct.unpack_from("<I", avi_file, at + 4)[0] == 16 * 30, (len(chunks), avi_file[at:at + 8])
        pictures = [c for c in chunks if c]
        assert chunks[0] and all(c[:2] == b"\xff\xd8" and c[-2:] == b"\xff\xd9" for c in pictures), "JPEG frames"
        assert len(set(pictures)) >= 2, "the clock changed while it was recorded"
        if shutil.which("ffprobe"):
            probe = Path(tempfile.gettempdir()) / "mind-core-rec.avi"
            probe.write_bytes(avi_file)
            info = subprocess.run(["ffprobe", "-v", "error", "-show_entries", "stream=codec_name,width,height,r_frame_rate,nb_frames", "-of", "csv=p=0", str(probe)],
                                  capture_output=True, text=True).stdout.strip()
            assert info == f"mjpeg,{width},{height},10/1,30", info
        camera_files(mtype, cam_still, cam_video)
        # After a reboot: the disk keeps its files, the RAM disk starts empty.
        subprocess.run(["mdel", "-i", part, "::/NvVars"], env=MTOOLS_ENV, capture_output=True)
        vm = VM(args, image.relative_to(ROOT).as_posix(), raw=True, snapshot=False)
        try:
            require(vm.command("cat data/notes.txt"), "line one")
            # With the screenshot, the recording, and the camera's still and video.
            require(vm.command("ls data"), f"6 ENTRIES, 6 FILES, {15 + size + len(avi_file) + len(mtype('data/cam.bmp')) + len(mtype('data/cam.avi'))} BYTES")
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
            assert stopped.index("vfs_server") < stopped.index(BOOT_DRIVER) < stopped.index("compositor"), stopped
            vm.expect("MIND CORE KERNEL: INIT STARTED", timeout=90)
            vm.expect("MIND> ", timeout=60)
            require(vm.command("cat data/reboot.txt"), "kept")
            vm.send("reboot -f\n")
            output = vm.expect("MIND CORE KERNEL: REBOOT VIA", timeout=60)
            assert "REBOOTING..." in output and "STOPPED" not in output, output
            vm.expect("MIND CORE KERNEL: INIT STARTED", timeout=90)
            vm.expect("MIND> ", timeout=60)
            require(vm.command("reboot now"), "USAGE: REBOOT [-F]")
            # Power off (issue 203): PSCI SYSTEM_OFF ends QEMU on aarch64; x86 has no ACPI sleep states yet.
            if vm.arch == "aarch64":
                vm.send("reboot -f --off\n")
                vm.expect("MIND CORE KERNEL: POWER OFF VIA PSCI SYSTEM_OFF", timeout=30)
                vm.process.wait(timeout=30)
            else:
                require(vm.command("reboot -f --off"), "ERROR: POWER OFF REFUSED")
        finally:
            vm.close()
            (Path(tempfile.gettempdir()) / f"mind-core-vfs-3-{args.cpus}cpu.log").write_text(vm.log)
        fsck_volume(image, start, fs_sectors)
        disks_check(args, image, Path(temp))
    print("PASS: vfs: files written to a raw FAT disk in data/ pass fsck.fat and read back with mtools and after a reboot; the RAM disk is empty after it; "
          f"screenshot writes the screen as a BMP ({width}x{height}) and the display's capture dot goes out after it; record writes 3 s of clock as AVI/MJPEG ({len(set(pictures))} pictures in 30 frames); reboot stops {len(stopped)} services and keeps an unsynced file; reboot -f; "
          f"{'power off' if args.arch == 'aarch64' else 'power off refused'}", flush=True)


def disks_check(args, boot, temp):
    """251-KRN-0031: a model disk and a store disk next to the boot disk each reach their own service: the blank VirtIO
    disk the block store (init routes it by its first sector), the FAT32 disk labelled MIND MODELS vfs_server as models:,
    and the boot volume mounts as before. On aarch64, where the boot disk is VirtIO too, that is three VirtIO disks."""
    models, store = temp / "models-disk.img", temp / "store-disk.img"
    (temp / "models-tree").mkdir()
    (temp / "models-tree" / "MANIFEST.json").write_bytes(b'{"models": []}\n')
    subprocess.run([sys.executable, str(ROOT / "scripts/fat32.py"), str(models), str(temp / "models-tree")], check=True, capture_output=True)
    with store.open("wb") as f:
        f.truncate(8 << 20)  # the store formats it at start: about 10 s on aarch64 (polled VirtIO under TCG) for 8 MiB
    extra = ["-drive", f"if=none,id=models,format=raw,readonly=on,file={models}", "-device", "virtio-blk-pci,drive=models",
             "-drive", f"if=none,id=store,format=raw,file={store}", "-device", "virtio-blk-pci,drive=store"]
    vm = VM(args, boot.relative_to(ROOT).as_posix(), raw=True, extra=extra)
    try:
        routed = re.search(r"\[INIT\] THE BLOCK STORE'S DISK: (virtio_blk(?:#\d)?) \(BLANK\)", vm.command("dmesg -s init", raw=True))
        assert routed, vm.log[-3000:]
        mounted = vm.command("dmesg -s vfs_server", raw=True)
        require(mounted, f"[VFS] MOUNTED FAT16 FROM {BOOT_DRIVE} AT LBA 2048")
        require(mounted, "[VFS] THE BOOT VOLUME:")
        require(mounted, "[VFS] MOUNTED FAT32 FROM VIRTIO AS MODELS:")
        assert re.search(r"^models: +MIND MODELS +FAT32 ", vm.command("df"), re.M), vm.log[-2000:]
        for _ in range(60):
            if "[BLOCKSTORE] READY" in vm.command("dmesg -s blockstore", raw=True):
                break
            time.sleep(2)
        assert re.search(r"SECTORS=\d+/16384", vm.command("blocks stat", raw=True)), vm.log[-2000:]
    finally:
        vm.close()
        (Path(tempfile.gettempdir()) / f"mind-core-disks-{args.cpus}cpu.log").write_text(vm.log)
    print(f"PASS: disks: the boot disk on {BOOT_DRIVE}, a model disk and a blank store disk on VirtIO: the store's goes to the "
          f"block store ({routed[1]}), the model disk to vfs_server as models:, the boot volume as before", flush=True)


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
    # T4: a Rust file in its language's colours on the classic blue, pixel for pixel: keyword, plain text, comment.
    require(vm.command("write ram:colour.rs fn main() // note"), "WROTE 18 BYTES")  # no braces: msh would read them
    start("ram:colour.rs")
    keys(b"\x1b[F", "LINE=1 COL=18 ")  # End: the cursor's underline off the cells checked
    time.sleep(.3)
    _, size, _, _ = vm.screenshot().split(b"\n", 3)
    width, height = map(int, size.split())
    x0, y0 = width % 8 // 2, height % 16 // 2 + 16
    check_text16(vm, x0, y0, "fn", 0xFFFFFF, 0x0000AA)
    check_text16(vm, x0 + 3 * 8, y0, "main", 0x55FFFF, 0x0000AA)
    check_text16(vm, x0 + 10 * 8, y0, "// note", 0xAAAAAA, 0x0000AA)
    assert canon("Rust") in screen_text(vm)[0]
    vm.serial(enter=False)
    leave(f10, "[EDIT] DONE")
    # 175-APP-0035: a save stages in a file of its own; an existing keep.txt.tmp stays as it was.
    require(vm.command("write ram:keep.txt.tmp not the editor's"), "WROTE")
    start("ram:keep.txt")
    keys(b"kept", "BYTES=4 ")
    keys(f2, "[EDIT] SAVED 4 BYTES TO ram:keep.txt")
    leave(f10, "[EDIT] DONE")
    require(vm.command("cat ram:keep.txt.tmp"), "not the editor's")
    require(vm.command("cat ram:keep.txt"), "kept")
    assert "keep.txt.tmp1" not in vm.command("ls ram:"), "the save's own staging file is gone"
    # vfs_server made a scope for each start and ended those whose editor had exited.
    scopes = vm.command("dmesg -s vfs_server")
    for made in ("FOR ram:/ (WRITABLE)", "FOR :/data (WRITABLE)", "FOR :/ (READ-ONLY)"):
        require(scopes, made)
    require(scopes, "ENDED")
    print("PASS: edit: Latin and Cyrillic text saved on ram: and in data/ (F2, the unsaved-changes dialog), read back; CRLF kept; a boot file opens read-only; the editor's client is confined to its file's directory; a Rust file in its colours; a save leaves an existing name.tmp alone", flush=True)


def edit_suite(args):
    """The editor on a raw FAT disk; the host then checks the image with fsck.fat and reads the files with mtools."""
    if not raw_tools():
        print("SKIP: edit suite needs mkfs.fat, fsck.fat and mtools", flush=True)
        return
    with tempfile.TemporaryDirectory(prefix="smoke-edit-", dir=ROOT / IMAGE) as temp:
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
    with tempfile.TemporaryDirectory(prefix="smoke-disk-", dir=ROOT / IMAGE) as temp:
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
        models_check(args, image, temp)


def models_check(args, boot, temp):
    """Issue 251: a model disk from scripts/fat32.py on a read-only VirtIO disk next to the boot disk is models:, read-only
    for the user too; sha256 in the system gives the host's hash of each file, and fsck finds it clean."""
    import hashlib
    tree, image = temp / "models", temp / "models.img"
    files = {"MANIFEST.json": b'{"models": []}\n', "asr-test/am-onnx/encoder.int8.onnx": bytes(range(256)) * (12 << 10) + b"tail",
             "tts-test/voice.bin": "голос".encode() * 999}  # commands go out as ASCII; tests/fat_host.rs covers Cyrillic names
    for path, data in files.items():
        (tree / path).parent.mkdir(parents=True, exist_ok=True)
        (tree / path).write_bytes(data)
    subprocess.run([sys.executable, str(ROOT / "scripts/fat32.py"), str(image), str(tree)], check=True, capture_output=True)
    vm = VM(args, boot.relative_to(ROOT).as_posix(), raw=True,
            extra=["-drive", f"if=none,id=models,format=raw,readonly=on,file={image}", "-device", "virtio-blk-pci,drive=models"])
    try:
        mounted = vm.service_logs("vfs_server", "AS RAM:")
        require(mounted, "[VFS] MOUNTED FAT16 FROM ATA AT LBA 2048")
        require(mounted, "[VFS] MOUNTED FAT32 FROM VIRTIO AS MODELS: (256 MB, READ-ONLY)")
        assert re.search(r"^models: +MIND MODELS +FAT32 +4096 ", vm.command("df"), re.M), vm.log[-2000:]
        command = "sha256 " + " ".join(f"models:{path}" for path in files)
        vm.send(command + "\n")
        hashes = vm.expect("MIND> ", timeout=120, after=to_ordinal(to_real(command)) + "\n")
        for path, data in files.items():
            require(hashes, f"{hashlib.sha256(data).hexdigest()}  models:{path}")
        for command, what in [("write models:new.txt text", "WRITE"), ("mkdir models:dir", "MKDIR"), ("rm models:MANIFEST.json", "RM")]:
            assert re.search(f"ERROR: {what}: (DENIED|READ-ONLY DEVICE)", vm.command(command)), vm.log[-2000:]
        require(vm.command("fsck models:"), "  clean")
    finally:
        vm.close()
    print("PASS: models: a FAT32 model disk on VirtIO, read-only; sha256 in the system matches the host for 3 files; writes refused; fsck clean", flush=True)


def block_suite(args, block_elf):
    """Block write through each driver: a raw FAT image (the file system in its first 60 MiB) boots with the test
    stand-in for vfs_server, which writes 8 sectors near the end of the disk through the write-badged client init
    gives it, flushes and reads them back; the image is then checked on the host."""
    if not raw_tools():
        print("SKIP: block suite needs mkfs.fat, fsck.fat and mtools", flush=True)
        return
    with tempfile.TemporaryDirectory(prefix="smoke-block-", dir=ROOT / IMAGE) as temp:
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


def updater_suite(args, updater_elf):
    """The updater's authorities (351-KRN-0022): a raw FAT image boots with the test stand-in for the updater, which
    reports the slots init filled (and no others) and what each authority does; when the shell makes data/reboot, it asks
    init to restart the machine, and init flushes the volumes, stops the services and resets (QEMU exits: -no-reboot).
    vfs_server writes directories through today, so the image shows the restart left the volume whole, not the flush."""
    if not raw_tools():
        print("SKIP: updater suite needs mkfs.fat, fsck.fat and mtools", flush=True)
        return
    with tempfile.TemporaryDirectory(prefix="smoke-updater-", dir=ROOT / IMAGE) as temp:
        policy = b"# the updater may reach the release server\nupdater 10.0.2.2 tcp 8443 3600 1048576\n"
        image, start, fs_sectors = raw_fat_image(Path(temp), {"updater.elf": updater_elf}, extra={"netpolicy.txt": policy})
        part = f"{image}@@{start * 512}"
        vm = VM(args, image.relative_to(ROOT).as_posix(), raw=True, snapshot=False, extra=["-nic", "none", "-netdev", "user,id=n0", "-device", "virtio-net-pci,netdev=n0"])
        try:
            report = vm.service_logs("updater", "[UPDATER-STUB] FIRMWARE")
            # Its own endpoint, the clock, the file system, init (badged for reboot), the log every service has, the
            # flow grant, TLS, the firmware privilege; nothing else.
            held = re.search(r"\[UPDATER-STUB\] HOLDS((?: \d+:\d+)*)", report)
            assert held and held[1] == " 1:1 2:1 3:1 11:1 12:1 18:1 20:1 27:16", report
            require(report, "[UPDATER-STUB] FIRMWARE READ VFS READ WRITE DENIED")
            for command, answer in [("mkdir data/kept", "OK"), ("mkdir data/reboot", "OK")]:
                require(vm.command(command), answer)
            vm.process.wait(timeout=60)
            assert "PANIC" not in vm.log, vm.log[-2000:]
        finally:
            vm.close()
            (Path(tempfile.gettempdir()) / f"mind-core-updater-{args.cpus}cpu.log").write_text(vm.log)
        fsck_volume(image, start, fs_sectors)
        listing = subprocess.run(["mdir", "-b", "-i", part, "::/data"], env=MTOOLS_ENV, capture_output=True, text=True).stdout
        assert "kept" in listing and "reboot" in listing, listing
    print("PASS: updater: init grants it exactly its authorities; its restart through init resets the machine with the volume whole", flush=True)


def tone_power(samples, rate, start, hz):
    """How much of `hz` the 40 ms of `samples` from `start` hold."""
    window = samples[start:start + int(rate * 0.04)]
    return abs(sum(x * complex(math.cos(2 * math.pi * hz * i / rate), -math.sin(2 * math.pi * hz * i / rate)) for i, x in enumerate(window))) / len(window)


def beep_demo_tones(left, rate, first):
    """beep's demo from sample `first`: 523, 659 and 784 Hz, 150 ms each, each stronger than the other two."""
    for index, hz in enumerate((523, 659, 784)):
        start = first + int(rate * (0.05 + 0.15 * index))
        assert tone_power(left, rate, start, hz) > 5 * max(tone_power(left, rate, start, other) for other in (523, 659, 784) if other != hz), hz


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
    # beep with notes (issue u005): a console program, no screen; frequency and duration pairs, 0 Hz a pause.
    require(vm.command("beep 10"), "BEEP: 10 HZ: A FREQUENCY IS 20-20000 HZ, OR 0 FOR A PAUSE")
    played = vm.command("beep 440 200 0 100 880 300")
    require(played, "[BEEP] PLAYED 3 NOTES, 600 MS")
    require(played, "[BEEP] DONE")
    assert task_rows(vm) == {}, "beep ended with its sound"
    vm.close()
    import struct, wave
    with wave.open(str(wav)) as audio:
        frames = audio.readframes(audio.getnframes())
        rate = audio.getframerate()
    left = struct.unpack(f"<{len(frames) // 2}h", frames)[0::2]
    loud = [i for i, sample in enumerate(left) if sample]
    assert loud, "AC97 produced no audio"
    # The demo, then after a second of silence the notes.
    gap = next(k for k in range(1, len(loud)) if loud[k] - loud[k - 1] > rate)
    demo, notes = loud[:gap], loud[gap:]
    seconds = (demo[-1] - demo[0]) / rate
    assert 0.8 < seconds < 1.3, seconds  # 3 tones of 150 ms + 0.5 s sweep

    def power(start, hz):
        return tone_power(left, rate, start, hz)
    beep_demo_tones(left, rate, demo[0])
    seconds = (notes[-1] - notes[0]) / rate
    assert 0.55 < seconds < 0.65, seconds  # 200 ms, a 100 ms pause, 300 ms
    first, second = notes[0] + int(rate * 0.08), notes[0] + int(rate * 0.4)
    assert power(first, 440) > 5 * power(first, 880) and power(second, 880) > 5 * power(second, 440)
    pause = notes[0] + int(rate * 0.22)
    assert not any(left[pause:pause + int(rate * 0.06)]), "the pause is silent"
    print("PASS: audio gateway: AC97 DMA ring, IRQ via IPC, tones 523/659/784 Hz and client PCM in captured audio; "
          "beep's notes (440 Hz, a pause, 880 Hz) without a screen", flush=True)


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


def ehci_suite(args):
    """211-DRV-0004, 0017, 0018: usb_host's EHCI driver, as on an Intel Mac: a high-speed keyboard on QEMU's usb-ehci
    with a tablet on xHCI, then the other way round. The keys arrive both times (the trackpad commit's queues once
    overflowed usb_host's stack as it set up EHCI, which QEMU's other suites, without EHCI, did not see)."""
    import copy
    pc = copy.copy(args)
    pc.machine = "pc,i8042=off"
    for name, extra in (("on EHCI", ["-device", "qemu-xhci,id=xhci", "-device", "usb-tablet,bus=xhci.0", "-device", "usb-ehci,id=ehci", "-device", "usb-kbd,bus=ehci.0,usb_version=2"]),
                        ("on xHCI", ["-device", "qemu-xhci,id=xhci", "-device", "usb-kbd,bus=xhci.0", "-device", "usb-ehci,id=ehci", "-device", "usb-tablet,bus=ehci.0,usb_version=2"])):
        vm = VM(pc, IMAGE, extra=extra)
        try:
            host = vm.command("dmesg -s usb_host", raw=True)
            require(host, "[USB] EHCI 0: 6 PORTS")
            require(host, "[USB] EHCI 0 0627:0001 ADDRESS 1 (HIGH SPEED)")
            vm.send("run keys\n"); vm.expect("[KEYS] READY"); time.sleep(.3)
            start = len(vm.log)
            for key in ("a", "b", "c"):
                vm.hmp(f"sendkey {key}"); time.sleep(.2)
            time.sleep(.8); vm.collect()
            got = re.findall(r"\[KEYS\] code=Char mods=- char=([abc])", vm.log[start:])
            assert got == ["a", "b", "c"], (name, vm.log[start:])
            vm.send_bytes(b"\x1b"); vm.expect("EXITED. SHELL RESUMED.")
        finally:
            vm.close()
    print("PASS: EHCI: a keyboard on EHCI with a tablet on xHCI types, and the other way round", flush=True)


def hda_suite(args):
    """551-DRV-0010: Intel HD Audio in audio_gw. QEMU's intel-hda with a duplex codec (a line out and a line in): the
    gateway finds the paths, beep's tones reach the wav backend through the output stream, and listen records a second
    from the input stream (the "none" backend feeds silence at the real rate)."""
    import wave
    with tempfile.TemporaryDirectory(prefix="mind-hda-") as temp:
        wav = Path(temp) / "hda.wav"
        vm = VM(args, IMAGE, audio=str(wav), audio_card="HDA")
        try:
            ready = vm.service_logs("audio_gw", "[AUDIO] HDA READY")
            found = re.search(r"\[AUDIO\] HDA READY: CODEC 0 ([0-9A-F]{4}):([0-9A-F]{4}) REVISION [0-9A-F]{8}; OUT PIN (0X[0-9A-F]+) \(LINE\) <- DAC (0X[0-9A-F]+); IN PIN (0X[0-9A-F]+) \(LINE\) -> ADC (0X[0-9A-F]+); 48000 HZ STEREO S16, 32 DMA BUFFERS, (INTERRUPTS|POLLED)", ready.upper())
            assert found, ready
            require(vm.command("run beep &"), "PID=1 NAME=beep BACKGROUND")
            output = ""
            for _ in range(60):
                output += vm.command("logs 1")
                if "[BEEP] DONE" in output:
                    break
                time.sleep(.2)
            require(output, "[BEEP] DEVICE=true RATE=48000")
            time.sleep(1.5)
        finally:
            vm.close()
        with wave.open(str(wav)) as audio:
            frames, rate = audio.readframes(audio.getnframes()), audio.getframerate()
        left = struct.unpack(f"<{len(frames) // 2}h", frames)[0::2]
        loud = [i for i, sample in enumerate(left) if sample]
        assert loud, "HDA produced no audio"
        beep_demo_tones(left, rate, loud[0])
        vm = VM(args, IMAGE, audio="none", audio_card="HDA")
        try:
            require(vm.command("run listen 1 &"), "PID=1 NAME=listen BACKGROUND")
            log = ""
            for _ in range(60):
                log += vm.command("logs 1")
                if "[LISTEN] DONE" in log:
                    break
                time.sleep(.25)
            require(log, "[LISTEN] RECORDED 48000 FRAMES (1000 MS)")
            require(log, "[LISTEN] PLAYED BACK")
        finally:
            vm.close()
    print(f"PASS: Intel HD Audio: codec {found[1]}:{found[2]}, line out {found[3]} <- DAC {found[4]}, line in {found[5]} -> ADC {found[6]} ({found[7].lower()}); "
          "beep's tones through the output stream, a second recorded from the input stream", flush=True)


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
    # say's screen (issue u010): its text in the 8x16 font, Cyrillic as it is, whole; it stays until Esc.
    vm.send("say\n")
    pid = re.findall(r"STARTED PID=(\d+) NAME=say FOREGROUND", vm.expect("[SAY] DONE", timeout=30))[-1]
    screen = screen_text(vm)
    assert any(canon("Привет. Я разум корабля. Система готова к работе. Hello world.") in row for row in screen), screen[:8]
    vm.serial(enter=False)  # the screenshot was taken in QEMU's monitor
    vm.send("\x1b\n")
    vm.expect(f"PID={pid} EXITED. SHELL RESUMED.")
    assert "FAULT PID=" not in vm.command("faults")
    print(f"PASS: microphone capture through audio_gw (48 kHz, AC97 PCM in, one owner), playback{' on an interrupt line shared with the network card, both drivers interrupted' if shared else ''}, program arguments, run by name, "
          f"speech detection on the microphone and in a WAV file ({len(starts)} phrases at {found} ms), "
          "voice commands recognized by hear, say's text on its screen (Cyrillic, whole), voice control in the shell (a tool started, the time spoken, a service stopped "
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


def policy_answer(vm, command, answer):
    """A `netpolicy add|remove` command, answered at its (Y/N) question; what it printed."""
    vm.send(command + "\n")
    vm.expect("(Y/N)")
    vm.send(answer)
    return vm.expect("MIND> ")


def policy_edit_check(vm, web_port):
    """108: the network policy changed while the system runs, only after the user agrees; the change is kept in the
    broker's private directory, which the shell cannot open, and the next grant follows it."""
    line = f"rogue 10.0.2.2 tcp {web_port}"
    require(vm.command("netpolicy"), f"named www.mind.test tcp {web_port}")
    require(policy_answer(vm, f"netpolicy add {line}", "n"), "NETPOLICY: NOT CHANGED")
    require(vm.command(f"rogue tcp:10.0.2.2:{web_port}"), "NETWORK FOR rogue: NoPolicy")
    require(policy_answer(vm, f"netpolicy add {line}", "y"), "NETPOLICY: ADDED 1 LINE(S)")
    require(vm.command("netpolicy"), line)
    require(vm.command(f"rogue tcp:10.0.2.2:{web_port}"), f"NETCHECK tcp:10.0.2.2:{web_port} OK")
    require(vm.command("cat system/netpolicy/netpolicy.txt"), "ERROR: CAT: DENIED")
    require(policy_answer(vm, "netpolicy add rogue nowhere", "y"), "NETPOLICY: Invalid")
    require(policy_answer(vm, f"netpolicy remove {line}", "y"), "NETPOLICY: REMOVED 1 LINE(S)")
    require(vm.command(f"rogue tcp:10.0.2.2:{web_port}"), "NETWORK FOR rogue: NoPolicy")
    log = vm.command("dmesg -s netpolicy")
    for expected in (f"POLICY CHANGED BY PID", f"ADDED {line}", f"REMOVED {line} (1 LINES)", "CHANGE REFUSED FOR PID", "NOT A POLICY LINE: rogue nowhere"):
        require(log, expected)
    print("PASS: the network policy changed while the system runs: refused without the user's yes, a line added and the next grant following it, "
          "a line that is not policy refused, a line removed; the changed policy in the broker's private directory, which the shell cannot open", flush=True)


def download_check(args, disk):
    """download (351-NET-0001): 30 MiB over HTTP into data/ through its own grant, the first response cut at 10 MiB and
    the rest asked for with Range; the SHA-256 checked in the system; a file already complete; a download given up on
    and resumed by the next run; what the grant, the server and the file's directory refuse. The same over HTTPS
    (351-NET-0002), the server trusted by its pinned key or by the roots. On x86 the boot disk is on AHCI: the IDE
    driver's port I/O, emulated, takes minutes for 30 MiB. vfs_server writes a sector per block request and walks the
    file's chain on every write, which emulated aarch64 takes over 10 minutes for 30 MiB: 8 MiB there."""
    files = Path(tempfile.mkdtemp(prefix="mind-download-"))
    certificates = Path(tempfile.mkdtemp(prefix="mind-download-tls-"))
    size, cut = (30 << 20, 10 << 20) if args.arch == "x86_64" else (8 << 20, 3 << 20)
    big, small = os.urandom(size), os.urandom(200_000)
    (files / "big.bin").write_bytes(big)
    (files / "small.bin").write_bytes(small)
    # The release server, the first response for each file cut short, and a malformed head for /bad.bin (test hooks).
    release = serve_release.serve(files, cuts={"/big.bin": cut, "/small.bin": 50_000}, raw={"/bad.bin": BAD_HEAD})
    port = release.server_address[1]
    # The same files over HTTPS: the test CA's server (in tlsroots.pem), and one from a CA nobody trusts.
    _certificates(certificates)
    shutil.copyfile(certificates / "ca.pem", disk / "tlsroots.pem")
    secure = serve_release.serve(files, certificates / "server.pem", certificates / "server.key", cuts={"/big.bin": cut})
    rogue = serve_release.serve(files, certificates / "rogue.pem", certificates / "rogue.key")
    tls_port, rogue_port = secure.server_address[1], rogue.server_address[1]
    (disk / "data").mkdir(exist_ok=True)
    (disk / "netpolicy.txt").write_text(f"# download may reach the release servers for an hour, up to 64 MiB\n"
                                        + "".join(f"download 10.0.2.2 tcp {p} 3600 {64 << 20}\n" for p in (port, tls_port, rogue_port)))
    # Scripts that lend download everything it asks for but the parser service (109-NET-0009), or but the TLS client.
    (disk / "noparse.msh").write_text(f"#!msh\nrequires: console network file\ndownload data/n.bin http://10.0.2.2:{port}/small.bin\n")
    (disk / "notls.msh").write_text(f"#!msh\nrequires: console network file parse\ndownload data/n.bin https://10.0.2.2:{tls_port}/small.bin\n")
    # TLS takes random bytes from RDRAND only.
    cpu = ["-cpu", "qemu64,+rdrand"] if args.arch == "x86_64" else []
    vm = VM(args, disk.relative_to(ROOT).as_posix(), ahci=args.arch == "x86_64", extra=[*cpu, "-nic", "none", "-netdev", "user,id=n0", "-device", "virtio-net-pci,netdev=n0"])
    try:
        download_runs(vm, release, port, big, small, cut)
        https_download_runs(vm, secure, tls_port, rogue_port, {name: _spki_pin(certificates / f"{name}.pem") for name in ("server", "rogue")}, big, small, cut)
    finally:
        vm.close()
        for server in (release, secure, rogue):
            server.shutdown()
        shutil.rmtree(files, ignore_errors=True)
        shutil.rmtree(certificates, ignore_errors=True)
        (Path(tempfile.gettempdir()) / f"mind-core-download-{args.cpus}cpu.log").write_text(vm.log)


def download_runs(vm, release, port, big, small, cut):
    url = f"http://10.0.2.2:{port}"
    size = len(big)

    def run(command, until, timeout=8):
        # The program's lines, up to the prompt after its last one.
        vm.send(command + "\n")
        return vm.expect("MIND> ", timeout=timeout, after=until)
    digest = hashlib.sha256(big).hexdigest()
    out = run(f"download data/big.bin {url}/big.bin --sha256 {digest}", f"DOWNLOAD: SHA256 {digest} MATCHES", timeout=900)
    for line in (f"DOWNLOAD: CONNECTION CUT AT {cut} OF {size}, RESUMING", f"DOWNLOAD: DONE {size} BYTES IN"):
        require(out, line)
    assert release.requests == [("/big.bin", None), ("/big.bin", f"bytes={cut}-")], release.requests
    out = run(f"download data/big.bin {url}/big.bin --sha256 {digest}", f"DOWNLOAD: SHA256 {digest} MATCHES", timeout=300)
    require(out, f"DOWNLOAD: RESUMING data/big.bin AT {size}"); require(out, f"DOWNLOAD: ALREADY COMPLETE, {size} BYTES")
    print(f"PASS: download: {size >> 20} MiB over HTTP through its own grant, the connection cut at {cut >> 20} MiB and resumed with Range; SHA-256 checked; a complete file left as it is", flush=True)
    out = run(f"download data/small.bin {url}/small.bin --tries 1", "DOWNLOAD: GAVE UP AFTER 1 CONNECTIONS AT 50000 BYTES")
    require(out, "DOWNLOAD: CONNECTION CUT AT 50000 OF 200000, RESUMING")
    small_digest = hashlib.sha256(small).hexdigest()
    out = run(f"download data/small.bin {url}/small.bin --sha256 {small_digest}", f"DOWNLOAD: SHA256 {small_digest} MATCHES")
    require(out, "DOWNLOAD: RESUMING data/small.bin AT 50000")
    assert release.requests[-1] == ("/small.bin", "bytes=50000-"), release.requests
    require(run(f"download data/x.bin {url}/missing.bin", "DOWNLOAD: HTTP: Status(404)"), "DOWNLOAD: HTTP: Status(404)")
    require(run(f"download data/x.bin http://10.0.2.2:{port + 1}/x --tries 1", "DOWNLOAD: GAVE UP"), "DOWNLOAD: CONNECT: Denied")
    require(run("download data/x.bin https://10.0.2.2/x", "DOWNLOAD: TLS"), "DOWNLOAD: TLS: Denied")
    require(run(f"download kernel.elf {url}/small.bin", "DOWNLOAD: CANNOT OPEN"), "DOWNLOAD: CANNOT OPEN kernel.elf")
    require(vm.command("dmesg -s netpolicy"), f"TO download: 3 RULES, 3600 S, {64 << 20} BYTES")  # the release servers over http, https and the untrusted one
    print("PASS: download: a run given up on is resumed by the next; a missing file, a port outside the grant (over http and https) and a file outside data/ refused", flush=True)
    parser_check(vm, url)


def _spki_pin(certificate):
    """The SHA-256 of a certificate's SubjectPublicKeyInfo (DER), in hex: what `download --pin` takes."""
    key = subprocess.run(["openssl", "x509", "-in", str(certificate), "-pubkey", "-noout"], check=True, capture_output=True).stdout
    der = subprocess.run(["openssl", "pkey", "-pubin", "-outform", "der"], input=key, check=True, capture_output=True).stdout
    return hashlib.sha256(der).hexdigest()


def https_download_runs(vm, secure, port, rogue_port, pins, big, small, cut):
    """351-NET-0002: download over a TLS session of the TLS service on its own flow grant, the server trusted by its
    pinned key alone or by the roots in tlsroots.pem; a wrong pin and an untrusted server refused; without the TLS
    client (a script that does not declare tls) https is refused."""
    url, size = f"https://10.0.2.2:{port}", len(big)

    def run(command, until, timeout=8):
        vm.send(command + "\n")
        return vm.expect("MIND> ", timeout=timeout, after=until)
    digest = hashlib.sha256(big).hexdigest()
    out = run(f"download data/tls.bin {url}/big.bin --pin {pins['server']} --sha256 {digest}", f"DOWNLOAD: SHA256 {digest} MATCHES", timeout=1200)
    for line in (f"DOWNLOAD: CONNECTION CUT AT {cut} OF {size}, RESUMING", f"DOWNLOAD: DONE {size} BYTES IN"):
        require(out, line)
    assert secure.requests == [("/big.bin", None), ("/big.bin", f"bytes={cut}-")], secure.requests
    log = vm.command("dmesg -s tls")
    require(log, "10.0.2.2 VERIFIED BY ITS PINNED KEY, SUITE")
    small_digest = hashlib.sha256(small).hexdigest()
    require(run(f"download ram:roots.bin {url}/small.bin --sha256 {small_digest}", "MATCHES", timeout=60), f"DOWNLOAD: SHA256 {small_digest} MATCHES")
    print(f"PASS: download over HTTPS: {size >> 20} MiB through a TLS session on its own grant, cut at {cut >> 20} MiB and resumed with Range, the server verified by its pinned key; another file verified by the roots", flush=True)
    rogue = f"https://10.0.2.2:{rogue_port}"
    require(run(f"download ram:x.bin {url}/small.bin --pin {'0' * 64}", "DOWNLOAD: TLS"), "DOWNLOAD: TLS: Certificate")
    require(run(f"download ram:x.bin {rogue}/small.bin", "DOWNLOAD: TLS"), "DOWNLOAD: TLS: Certificate")
    # A pinned key is trusted whoever signed its certificate.
    require(run(f"download ram:rogue.bin {rogue}/small.bin --pin {pins['rogue']} --sha256 {small_digest}", "MATCHES", timeout=60), f"DOWNLOAD: SHA256 {small_digest} MATCHES")
    require(run(f"download ram:x.bin http://10.0.2.2:{port}/small.bin --pin {pins['server']}", "DOWNLOAD:"), "DOWNLOAD: --pin IS FOR https:// URLS")
    log = vm.command("dmesg -s tls")
    require(log, "10.0.2.2 REFUSED: NOT THE PINNED KEY"); require(log, "10.0.2.2 REFUSED: CERTIFICATE UnknownIssuer")
    require(vm.command("msh notls.msh"), "DOWNLOAD: HTTPS NEEDS A TLS CLIENT, AND NONE WAS LENT")
    print("PASS: download over HTTPS: a wrong pin and a server from an untrusted CA refused; a pinned key trusted whoever signed it; without the TLS client (a script without tls) https refused", flush=True)


BAD_HEAD = b"HTTP/1.1 200 OK\r\nContent-Length: twelve\r\nContent-Type: text/plain\r\n\r\nhello, world"


def parser_check(vm, url):
    """109-NET-0008, 0009: every response head above was parsed in `parse`, which holds its endpoint and the log client
    and nothing else; a malformed head is refused there, and download, given everything but the parser, refuses."""
    out = vm.command(f"download data/bad.bin {url}/bad.bin --tries 1")
    require(out, "DOWNLOAD: HTTP: Head")
    real = vm.services()
    log, head = vm.command("dmesg -s parse"), len(BAD_HEAD.split(b"\r\n\r\n")[0])
    assert re.search(fr"\[PARSE\] REFUSED AN HTTP HEAD FOR PID \d+: MALFORMED \({head} BYTES\)", log), log
    # Its capabilities: its own endpoint (served by it) and a client of logd; no memory, device, privilege or other client.
    servers = {int(ep): int(server) for ep, server in re.findall(r"^EP=(\d+) .*SERVER=(\d+)", vm.command("endpoints", raw=True), re.M)}
    text = vm.command(f"stat caps {real['parse']}", raw=True)
    caps = re.findall(r"^SLOT=(\d+) GEN=\d+ KIND=(\d+) .*?EP=(\d+)", text, re.M)
    held = sorted((int(slot), int(kind), servers.get(int(ep))) for slot, kind, ep in caps)
    require(text, "STAT CAPS VERSION=2 COUNT=2 ")
    assert held == [(1, 1, real["parse"]), (12, 1, real["logd"])], (held, text)
    out = vm.command("msh noparse.msh")
    require(out, "DOWNLOAD: NO PARSER SERVICE")
    # Killed, it is restarted by init behind the same endpoint: the shell's client reaches the new instance.
    require(vm.command(f"kill {real['parse']}", raw=True), f"KILLED PID={real['parse']}")
    for _ in range(40):
        if vm.services().get("parse", real["parse"]) != real["parse"]:
            break
        time.sleep(.25)
    assert vm.services()["parse"] != real["parse"], vm.services()
    out = vm.command(f"download ram:again.bin {url}/small.bin")
    require(out, "DOWNLOAD: DONE 200000 BYTES")
    print("PASS: parse: download's heads parsed in a service that holds only its endpoint and the log; a malformed head refused and logged there; download without it refuses; restarted after a kill, it serves again", flush=True)


def net_suite(args, disk):
    # Network card driver and stack in ring 3: DHCP, ICMP echo, DNS, TCP (HTTP) through QEMU's user-mode network,
    # raw frames from the driver, restart of the driver after device quiesce and of the stack.
    web = socketserver.ThreadingTCPServer(("127.0.0.1", 0), _Http)
    web.daemon_threads = True
    threading.Thread(target=web.serve_forever, daemon=True).start()
    dns = _dns_server()
    web_port, dns_port = web.server_address[1], dns.getsockname()[1]
    # The policy broker's file: netcheck may reach the host's web server and ping the gateway; rogue (a copy) nothing;
    # named (another copy) the web server by its name, looked up at the test DNS server (351-NET-0003).
    (disk / "netpolicy.txt").write_text(f"# test policy\nnetcheck 10.0.2.2 tcp {web_port} 600 100000\nnetcheck 10.0.2.2 icmp\nconsole 10.0.2.2 icmp\n"
                                        f"resolver 10.0.2.2:{dns_port}\nnamed www.mind.test tcp {web_port}\nnamed missing.example tcp {web_port}\n")
    shutil.copyfile(disk / "netcheck.elf", disk / "rogue.elf")
    shutil.copyfile(disk / "netcheck.elf", disk / "named.elf")
    vm = VM(args, disk.relative_to(ROOT).as_posix(), extra=["-nic", "none", "-netdev", "user,id=n0", "-device", "virtio-net-pci,netdev=n0"])
    try:
        log = vm.service_logs("virtio_net", "[VIRTIO_NET] MAC=")
        require(log, "[VIRTIO_NET] MAC=52:54:00:12:34:56 LINK=UP QUEUES=256/256 MODERN MSI-X")
        # The boot report of legacy hardware (x86): the transitional card needs no legacy code; the PIIX IDE controller does.
        if args.arch == "x86_64":
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
        vm.program_logs(holder, "NETCHECK HOLDING", 20)
        require(vm.command("netgrants"), "netcheck RULES=2")
        require(vm.command("netrevoke netcheck"), "REVOKED 1 GRANTS OF netcheck")
        for _ in range(20):
            ended = re.search(fr"NETCHECK hold:10.0.2.2:{web_port}:30 (NO GRANT|Denied|NoSocket|Closed)", vm.command(f"logs {holder}"))
            if ended:
                break
            time.sleep(.25)
        assert ended, vm.command(f"logs {holder}")
        require(vm.command("dmesg -s netpolicy"), "[NETPOLICY] REVOKED 2 OF netcheck: 1 COPIES REMOVED, 1 SOCKETS CLOSED")
        # Names in the policy (351-NET-0003): the address a name resolved to when the grant was made, and nothing else.
        checks = vm.command(f"named tcp:10.0.2.2:{web_port} tcp:10.0.2.3:{web_port}")
        require(checks, f"NETCHECK tcp:10.0.2.2:{web_port} OK"); require(checks, f"NETCHECK tcp:10.0.2.3:{web_port} Denied")
        log = vm.command("dmesg -s netpolicy")
        for line in ("[NETPOLICY] named: www.mind.test IS 10.0.2.2", "[NETPOLICY] named: missing.example NOT RESOLVED (NotFound)", "TO named: 1 RULES, 3600 S"):
            require(log, line)
        policy_edit_check(vm, web_port)
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
        # console's ping (issue u006): through its own grant, to what the policy names for it and nothing else.
        vm.send("console ping 10.0.2.2\n")
        vm.expect("[CONSOLE] READY")
        for _ in range(40):
            time.sleep(.25)
            screen = screen_text(vm)
            vm.serial(enter=False)
            if any(row.startswith(canon("ping: 3 sent")) for row in screen):
                break
        assert any(row.rstrip() == canon("ping: 3 sent, 3 received") for row in screen), screen
        vm.send_bytes(b"ping 10.0.2.3\r")
        time.sleep(1)
        screen = screen_text(vm)
        vm.serial(enter=False)
        assert any(row.startswith(canon("ping: 10.0.2.3: not allowed for console")) for row in screen), screen
        vm.send_bytes(b"exit\r")
        require(vm.expect("EXITED. SHELL RESUMED."), "[CONSOLE] DONE")
        require(vm.command("dmesg -s netpolicy"), "TO console: 1 RULES")
    finally:
        vm.close()
        web.shutdown(); dns.close()
        (Path(tempfile.gettempdir()) / f"mind-core-net-{args.cpus}cpu.log").write_text(vm.log)
    download_check(args, disk)
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
    # A modern-only card (no legacy registers) and a legacy-only one (no modern structures, no MSI-X; x86 only: the
    # aarch64 driver has no port I/O interface).
    cards = [("virtio-net-pci,netdev=n0,disable-legacy=on", "MODERN MSI-X"), ("virtio-net-pci,netdev=n0,disable-modern=on", "LEGACY INTX")]
    for device, mode in cards[:1] if args.arch == "aarch64" else cards:
        vm = VM(args, disk.relative_to(ROOT).as_posix(), extra=["-nic", "none", "-netdev", "user,id=n0", "-device", device])
        try:
            require(vm.service_logs("virtio_net", "[VIRTIO_NET] MAC="), mode)
            if args.arch == "x86_64":
                report = vm.service_logs("init", "LEGACY DEVICES FOUND")
                require(report, "[INIT] LEGACY VIRTIO DEVICE WITH ONLY THE LEGACY INTERFACE: " + ("FOUND 1" if mode == "LEGACY INTX" else "NOT FOUND"))
                assert ("VIRTIO TRANSITIONAL DEVICES" in report) is False, report
            require(vm.service_logs("netstack", "[NETSTACK] DHCP"), "[NETSTACK] DHCP 10.0.2.15/24")
            require(vm.command("ping 10.0.2.2"), "PING: 3 SENT, 3 RECEIVED")
            if mode == "MODERN MSI-X":
                _msix_only(vm)
        finally:
            vm.close()
    # QEMU's default e1000 (on x86; named on virt, whose default card is VirtIO) has the same PCI class: it is not taken
    # for a VirtIO card; the stack reports no network.
    vm = VM(args, disk.relative_to(ROOT).as_posix(), extra=["-nic", "user,model=e1000"] if args.arch == "aarch64" else ())
    try:
        assert "virtio_net" not in vm.services()
        require(vm.service_logs("init", "virtio_net NOT STARTED"), "virtio_net NOT STARTED: NO DEVICE")
        require(vm.command("net"), "NET: NO NETWORK CARD")
        require(vm.command("ip"), "IP: NoNetwork")
    finally:
        vm.close()
    print("PASS: VirtIO network card and network stack in ring 3: DHCP, ping, DNS, TCP/HTTP, refused connection, "
          "flow grants of the policy broker (allowed, denied, no policy, dropped at exit, revoked, a host name resolved at the grant), two cards on two networks "
          "(a driver instance and an interface each, routes by network, one driver restarted with its own card), "
          "raw ARP through the driver, restarts of the stack and of the driver after device quiesce; modern interface with MSI-X "
          f"(transitional and modern-only cards){'' if args.arch == 'aarch64' else ', legacy interface'}; e1000 not taken", flush=True)


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
    # Copies of clock that also ask for the network and the TLS client (351-APP-0017): tlsclock has a rule in the
    # policy, tlsnone none; a script that declares only `network` starts tlsclock too.
    elf = bytearray((disk / "clock.elf").read_bytes())
    note = elf.index(b"MINDREQ1") + 8
    elf[note:note + 4] = (int.from_bytes(elf[note:note + 4], "little") | 64 | 131072).to_bytes(4, "little")
    for name in ("tlsclock", "tlsnone"):
        (disk / f"{name}.elf").write_bytes(elf)
    (disk / "netpolicy.txt").write_text(f"tlsclock 10.0.2.2 tcp {ports['good']} 600 100000\n")
    (disk / "data").mkdir(exist_ok=True)
    (disk / "data/notls.msh").write_text("#!msh\nrequires: network\nrun tlsclock &\n")
    (disk / "data/tls.msh").write_text("#!msh\nrequires: network tls\nrun tlsclock &\n")
    # A processor with the random number instruction, and one without it (an ARMv8.0 core has no RNDR).
    entropy, (with_entropy, without) = ("RNDR", ("max", "cortex-a72")) if args.arch == "aarch64" else ("RDRAND", ("qemu64,+rdrand", None))
    vm = VM(args, disk.relative_to(ROOT).as_posix(), rtc="utc", extra=[*network, "-cpu", with_entropy])
    try:
        require(vm.service_logs("keystore", "DEVICE KEY READY"), "[KEYSTORE] DEVICE KEY READY: MIND ")
        require(vm.service_logs("tls", "[TLS] READY"), f"[TLS] READY: TLS 1.3 CLIENT, ROOTS FROM tlsroots.pem, RANDOM FROM {entropy}")
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
        tls_lending_check(vm, servers, real["tls"])
    finally:
        vm.close()
        for server in (good, rogue, asking):
            server.shutdown()
        (Path(tempfile.gettempdir()) / f"mind-core-tls-{args.cpus}cpu.log").write_text(vm.log)
    vm = VM(args, disk.relative_to(ROOT).as_posix(), rtc="utc", extra=[*network, *(["-cpu", without] if without else [])])
    try:
        require(vm.service_logs("keystore", f"NO {entropy}"), f"[KEYSTORE] NO {entropy}: NO DEVICE KEY")
        require(vm.service_logs("tls", "[TLS] READY"), f"NO {entropy}: EVERY CONNECTION WILL BE REFUSED")
        require(vm.command(f"https 10.0.2.2:{ports['good']} / mind.test"), "HTTPS: NoEntropy")
        require(vm.command("tls cert"), "TLS: NotFound")
    finally:
        vm.close()
    shutil.rmtree(certificates, ignore_errors=True)
    print("PASS: TLS 1.3 client service: HTTPS with the server certificate verified (by name and by address; AES-256-GCM, "
          "AES-128-GCM and ChaCha20-Poly1305; X25519 and P-256), wrong name, "
          "untrusted CA and refused port reported; the device certificate offered with -c and signed for by the key service, "
          f"which only the TLS service may ask; no {entropy}: no key and no connection", flush=True)
    device_key_check(args, with_entropy)
    tpm_check(args, with_entropy)


def device_key_check(args, cpu):
    """351-NET-0005: the device key kept across boots in the key service's private directory of the boot disk, on a raw
    image booted three times. The first boot makes and stores it, the second finds the same key, and a stored key damaged
    from the host is replaced. The shell cannot read the directory, and its public key is logged in the OpenSSH form."""
    if not raw_tools():
        print("SKIP: the device key check needs mkfs.fat, fsck.fat and mtools", flush=True)
        return
    with tempfile.TemporaryDirectory(prefix="smoke-key-", dir=ROOT / IMAGE) as temp:
        image, start, fs_sectors = raw_fat_image(Path(temp))
        part = f"{image}@@{start * 512}"

        def boot():
            vm = VM(args, image.relative_to(ROOT).as_posix(), raw=True, snapshot=False, rtc="utc",
                    extra=["-cpu", cpu, "-nic", "none", "-netdev", "user,id=n0", "-device", "virtio-net-pci,netdev=n0"])
            try:
                log = vm.service_logs("keystore", "PUBLIC KEY")
                name = re.search(r"\[KEYSTORE\] DEVICE KEY READY: MIND ([0-9A-F]{8}) ", log)
                assert name, log
                public = re.search(r"\[KEYSTORE\] PUBLIC KEY ssh-ed25519 ([A-Za-z0-9+/]{68}) MIND-([0-9A-F]{8})", log)
                assert public and public[2] == name[1], log
                # The blob of the OpenSSH key: the key type, then the key, whose first four bytes name the device.
                blob = base64.b64decode(public[1])
                assert blob[:15] == b"\x00\x00\x00\x0bssh-ed25519" and blob[15:19] == b"\x00\x00\x00\x20" and blob[19:23].hex().upper() == name[1], blob
                return vm, log, name[1]
            except BaseException:
                vm.close()
                raise
        vm, log, first = boot()
        try:
            require(log, "[KEYSTORE] DEVICE KEY MADE AND STORED IN system/keystore/device.key (ON DISK, NOT SEALED)")
            require(log, "[KEYSTORE] NO TPM: THE DEVICE KEY IS KEPT ON DISK, NOT SEALED")
            # The shell's client lists system/ but opens nothing below it, and writes nothing there.
            require(vm.command("ls system"), "keystore")
            require(vm.command("cat system/keystore/device.key"), "ERROR: CAT: DENIED")
            require(vm.command("ls system/keystore"), "DENIED")
            require(vm.command("write system/keystore/device.key x"), "ERROR: WRITE: DENIED")
            # 108: a change of the network policy is kept on the disk too.
            require(policy_answer(vm, "netpolicy add kept 10.0.2.2 tcp 7", "y"), "NETPOLICY: ADDED 1 LINE(S)")
        finally:
            vm.close()
        vm, log, second = boot()
        try:
            require(vm.command("netpolicy"), "kept 10.0.2.2 tcp 7")
        finally:
            vm.close()
        require(log, "[KEYSTORE] DEVICE KEY FROM system/keystore/device.key (ON DISK, NOT SEALED)")
        assert second == first, (first, second)
        damaged = Path(temp) / "device.key"
        damaged.write_bytes(b"MINDKEY1" + bytes(64))
        subprocess.run(["mcopy", "-o", "-i", part, str(damaged), "::/system/keystore/device.key"], env=MTOOLS_ENV, check=True, capture_output=True)
        vm, log, third = boot()
        vm.close()
        require(log, "[KEYSTORE] DEVICE KEY MADE ANEW: THE STORED ONE WAS DAMAGED AND STORED IN system/keystore/device.key")
        assert third != first, (first, third)
        fsck_volume(image, start, fs_sectors)
    print(f"PASS: the device key kept across boots in the key service's private directory (MIND {first} twice, a damaged one replaced); "
          "the shell can neither read nor write it; the public key logged in the OpenSSH form; a change of the network policy kept after a reboot", flush=True)


def tls_lending_check(vm, servers, tls):
    """351-APP-0017: the shell lends its TLS client in SLOT_TLS to a program that asks for it (REQUEST_TLS) and gets a
    flow grant; not without a grant, and to a script's program only if the script declares `tls`."""
    service = {ep for ep, server in servers.items() if server == tls}

    def lent(command):
        out = vm.command(command)
        pid = int(re.search(r"STARTED PID=(\d+) NAME=\w+ BACKGROUND", out)[1])  # as the suites number them
        caps = vm.command(f"stat caps {pid + BASE}", raw=True)
        require(vm.command(f"kill {pid}"), "KILLED")
        slot = re.search(r"^SLOT=20 [^\n]* EP=(\d+)", caps, re.M)
        return out, bool(slot and int(slot[1]) in service)

    out, tls_lent = lent("run tlsclock &")
    assert tls_lent, out
    require(vm.command("netgrants"), "GRANT ")
    out, tls_lent = lent("run tlsnone &")
    require(out, "NETWORK FOR tlsnone: NoPolicy")
    assert not tls_lent, "no flow grant, no TLS client"
    out, tls_lent = lent("msh data/notls.msh")
    assert not tls_lent, "a script without `tls`"
    out, tls_lent = lent("msh data/tls.msh")
    assert tls_lent, out
    print("PASS: the shell lends its TLS client to a program with a flow grant that asks for it, not to one without a "
          "grant, and to a script's program only if the script declares tls", flush=True)


def tpm_check(args, cpu):
    """351-DRV-0015, 351-KRN-0043, 351-NET-0006: a TPM 2.0 (swtpm; a CRB on x86, the FIFO of tpm-tis-device on aarch64)
    driven by the TPM service, which init gives its registers from the firmware's tables. On one raw image: a boot without
    a TPM keeps the device key unencrypted (the interim); a boot with TPM A seals that key and removes the plain file; A
    again unseals the same key; another TPM, B, does not open the blob, and a new key is made and sealed. The shell's
    client, without the seal badge, is refused a seal."""
    if not raw_tools() or not shutil.which("swtpm"):
        print("SKIP: the TPM check needs swtpm, mkfs.fat, fsck.fat and mtools", flush=True)
        return
    with tempfile.TemporaryDirectory(prefix="smoke-tpm-", dir=ROOT / IMAGE) as temp:
        temp = Path(temp)
        image, start, fs_sectors = raw_fat_image(temp)
        part = f"{image}@@{start * 512}"
        device = "tpm-tis-device" if args.arch == "aarch64" else "tpm-crb"

        def boot(tpm=None):
            # A swtpm for each boot, on the state directory of TPM `tpm`.
            swtpm, extra, sockets = None, [], None
            if tpm:
                (temp / tpm).mkdir(exist_ok=True)
                # A UNIX socket's path is under 108 bytes: in a short directory of its own, not beside a deep checkout.
                sockets = Path(tempfile.mkdtemp(prefix="tpm-", dir="/tmp"))
                socket_path = sockets / f"{tpm}.sock"
                swtpm = subprocess.Popen(["swtpm", "socket", "--tpm2", "--tpmstate", f"dir={temp / tpm}", "--ctrl", f"type=unixio,path={socket_path}", "--flags", "startup-clear"],
                                         stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
                for _ in range(50):
                    if socket_path.exists():
                        break
                    time.sleep(.1)
                extra = ["-chardev", f"socket,id=chrtpm,path={socket_path}", "-tpmdev", "emulator,id=tpm0,chardev=chrtpm", "-device", f"{device},tpmdev=tpm0"]
            vm = VM(args, image.relative_to(ROOT).as_posix(), raw=True, snapshot=False, rtc="utc", extra=["-cpu", cpu, *extra])
            vm.swtpm, vm.sockets = swtpm, sockets
            log = vm.service_logs("keystore", "PUBLIC KEY")
            name = re.search(r"\[KEYSTORE\] DEVICE KEY READY: MIND ([0-9A-F]{8}) ", log)
            assert name, log
            return vm, log, name[1]

        def close(vm):
            vm.close()
            if vm.swtpm:
                vm.swtpm.kill(); vm.swtpm.wait()
            if vm.sockets:
                shutil.rmtree(vm.sockets, ignore_errors=True)

        def stored(name):
            return subprocess.run(["mtype", "-i", part, f"::/system/keystore/{name}"], env=MTOOLS_ENV, capture_output=True).returncode == 0
        vm, log, plain = boot()
        try:
            require(log, "[KEYSTORE] NO TPM: THE DEVICE KEY IS KEPT ON DISK, NOT SEALED")
            require(vm.service_logs("tpm", "[TPM] NO TPM"), "[TPM] NO TPM")
            require(vm.command("tpm"), "TPM: NONE")
        finally:
            close(vm)
        assert stored("device.key") and not stored("device.sealed")
        vm, log, sealed = boot("a")
        try:
            interface = "FIFO" if args.arch == "aarch64" else "CRB"
            # The kernel hands out the TPM's registers from the TPM2 table (x86) or the DSDT (aarch64) (351-KRN-0052).
            ready = vm.service_logs("tpm", "[TPM] READY")
            require(ready, f"[TPM] READY: TPM 2.0 BY IBM, {interface} INTERFACE")
            require(log, "[KEYSTORE] DEVICE KEY FROM system/keystore/device.key SEALED BY THE TPM IN system/keystore/device.sealed; THE UNENCRYPTED COPY REMOVED")
            require(vm.command("tpm"), f"TPM 2.0 BY IBM, {interface} INTERFACE")
            require(vm.command("tpm seal not mine"), "TPM: Rights")
            log = vm.command("dmesg -s tpm")
            assert re.search(r"\[TPM\] SEALED 32 BYTES FOR PID \d+", log) and re.search(r"\[TPM\] REFUSED SEAL FOR PID \d+ \(BADGE 0\)", log), log
        finally:
            close(vm)
        assert sealed == plain, (plain, sealed)
        assert stored("device.sealed") and not stored("device.key")
        vm, log, again = boot("a")
        close(vm)
        require(log, "[KEYSTORE] DEVICE KEY FROM system/keystore/device.sealed (SEALED BY THE TPM)")
        assert again == plain, (plain, again)
        # The disk without its TPM: another TPM does not open the blob.
        vm, log, other = boot("b")
        close(vm)
        require(log, "[KEYSTORE] DEVICE KEY MADE ANEW: THE SEALED ONE DOES NOT OPEN ON THIS TPM AND SEALED BY THE TPM IN system/keystore/device.sealed")
        assert other != plain, (plain, other)
        fsck_volume(image, start, fs_sectors)
    print(f"PASS: TPM 2.0 ({device}, swtpm) driven by the TPM service: the unencrypted device key (MIND {plain}) sealed and its file removed, "
          "unsealed again by the same TPM, not opened by another (a new key made and sealed); the shell's client refused a seal", flush=True)


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

    def last_state():
        return re.findall(r"\[WM\] (MODE=[^\n]*)", "".join(seen))[-1]

    def full_screen_and_list(fm, clock, top):
        # Full screen (211-APP-0014): Alt+F gives the window in front the whole screen, without its frame or the bars;
        # Alt+Tab from it shows the desktop with the next window in front, and back it is full again; Alt+F again
        # gives it its frame back.
        frames = front(clock)[2]
        mode, focus, rects = keys("alt-f", text=f"FULL={clock}")
        assert focus == clock and rects == frames, (focus, rects)
        until(f"[WM] PIXELS {clock} 1280X800")  # it draws at the screen's size
        time.sleep(.5)
        screen = screen_text(vm)
        _, size, _, pixels = vm.screenshot().split(b"\n", 3)
        vm.serial(enter=False)
        width = int(size.split()[0])
        assert not screen[0].startswith(canon(" wm │")) and canon("keys go to") not in screen[-1], (screen[0], screen[-1])
        green = [px for py in range(0, 800, 2) for px in range(0, 1280, 2) if pixels[(py * width + px) * 3:(py * width + px) * 3 + 3] == bytes((0xA6, 0xE3, 0xA1))]
        assert green and max(green) > 700, (len(green), max(green, default=0))  # its digits across the screen
        mode, focus, rects = keys("alt-tab")
        assert focus != clock and "FULL=" not in last_state(), last_state()
        screen = screen_text(vm)
        vm.serial(enter=False)
        assert screen[0].startswith(canon(" wm │")), screen[0]
        keys(*["alt-tab"] * (len(frames) - 1), text=f"FULL={clock}")
        mode, focus, rects = keys("alt-f")
        assert focus == clock and rects == frames and "FULL=" not in last_state(), last_state()
        until(f"[WM] PIXELS {clock} {(frames[clock][2] - 2) * 8}X{(frames[clock][3] - 2) * 16}")
        # A text window: top on the whole cell grid, then back in its frame.
        front(top)
        keys("alt-f", text=f"FULL={top}")
        for _ in range(20):  # top lays itself out again on the whole grid
            time.sleep(.3)
            screen = screen_text(vm)
            vm.serial(enter=False)
            if any(canon("PID NAME") in row for row in screen):
                break
        assert not screen[0].startswith(canon(" wm │")) and any(canon("PID NAME") in row for row in screen), screen
        assert canon("keys go to") not in screen[-1], screen[-1]
        mode, focus, rects = keys("alt-f")
        assert focus == top and rects == frames and "FULL=" not in last_state(), last_state()
        # The window list (211-APP-0014): Alt+L lists the three windows with their programs' PIDs; the second is
        # brought to the front by keys, then by a click; a window closed from the list leaves it.
        ids = sorted(frames)
        owners = {int(m[0]): int(m[1]) for m in windows_re.findall("".join(seen))}
        mode, focus, rects = keys("alt-l", text="MODE=LIST")
        assert last_state().endswith(f"LIST={top}"), last_state()
        time.sleep(.3)
        screen = screen_text(vm)
        vm.serial(enter=False)
        title = next(i for i, row in enumerate(screen) if canon(" Windows ") in row)
        column = screen[title].index(canon(" Windows "))
        for row, window in zip(screen[title + 1:title + 1 + len(ids)], ids):
            assert canon(f"PID {owners[window] + BASE} ") in row, (window, row)
        assert canon("in front") in screen[title + 1 + ids.index(top)], screen[title + 1:title + 4]
        keys("home", "down", text=f"LIST={ids[1]}")
        mode, focus, rects = keys("ret")
        assert mode == "NORMAL" and focus == ids[1], (mode, focus)
        front(next(w for w in ids if w != ids[1]))
        keys("alt-l", text="MODE=LIST")
        point(column + 3, title + 2)  # the second entry
        mode, focus, rects = mouse("mouse_button 1", "mouse_button 0", lines=2)
        assert mode == "NORMAL" and focus == ids[1], last_state()
        # A fourth window, closed from the list with Alt+W: the list follows it out.
        keys("alt-r", "c", "l", "o", "c", "k", "ret", text="STARTED clock")
        while len(state()[2]) < 4:
            wait()
        extra = max(state()[2])
        keys("alt-l", "end", text=f"LIST={extra}")
        keys("alt-w", text=f"CLOSE {extra}")
        until(f"GONE {extra}")
        while extra in state()[2]:
            wait()
        assert last_state().startswith("MODE=LIST") and not last_state().endswith(f"LIST={extra}"), last_state()
        screen = screen_text(vm)
        vm.serial(enter=False)
        title = next(i for i, row in enumerate(screen) if canon(" Windows ") in row)
        assert sum(canon("PID ") in row for row in screen[title + 1:title + 6]) == 3, screen[title:title + 6]
        mode, focus, rects = keys("esc")
        assert mode == "NORMAL" and set(rects) == set(frames), (mode, rects)
        print("PASS: wm full screen: the clock's pixels and top's cells on the whole screen without frames or bars, Alt+Tab "
              "from it and back, Alt+F restoring the frame; the window list: PIDs and states, Enter and a click bring a "
              "window to the front, Alt+W closes one and the list follows", flush=True)

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
    # wm has passed the keys on; fm may still be changing the directory (its log is drained at each read).
    fm_log = ""
    for _ in range(20):
        fm_log += vm.command(f"logs {started['fm']}")
        if "LEFT=/docs FULL" in fm_log:
            break
        time.sleep(.25)
    require(fm_log, "CMD=cd docs")
    require(fm_log, "LEFT=/docs FULL")
    assert "[TOP] " not in vm.command(f"logs {started['top']}").replace("[TOP] READY", ""), "top got no key"
    vm.send(f"fg {wm_pid}\n")
    vm.expect(f"FOREGROUND PID={wm_pid}")
    # A program fm starts opens a window of its own next to fm's (issue 099), in front; Alt+W closes it.
    read[0] = len(vm.log)
    vm.send_bytes(b"clock\r")
    wait(lines=6)
    while not any(m[1] not in started.values() and m[2] == "PIXELS" for m in windows_re.findall("".join(seen))):
        wait("[WM] WINDOW", lines=0)
    second_clock = next(int(m[0]) for m in windows_re.findall("".join(seen)) if m[1] not in started.values() and m[2] == "PIXELS")
    while state()[1] != second_clock:
        wait()
    keys("alt-w", text=f"CLOSE {second_clock}")
    until(f"GONE {second_clock}")
    while second_clock in state()[2]:
        wait()
    assert state()[1] == fm, state()
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
    # A pixel window's content follows its frame (issue u009): the clock draws again at the pixels of the left half,
    # its digits wider than the 320 pixels it had.
    until(f"[WM] PIXELS {clock} 624X736")
    time.sleep(.5)
    _, size, _, pixels = vm.screenshot().split(b"\n", 3)
    vm.serial(enter=False)
    green = [px for py in range(2 * 16, 48 * 16) for px in range(8, 79 * 8) if pixels[(py * width + px) * 3:(py * width + px) * 3 + 3] == bytes((0xA6, 0xE3, 0xA1))]
    assert green and max(green) > 8 + 400, (len(green), max(green, default=0))
    # The mouse in a window (issue u001): with fm on the right half behind the clock, a click on an entry of fm's
    # brings its window to the front and goes to fm at the cell of its content; a double click on ".." opens it; the
    # wheel moves fm's cursor.
    front(fm)
    keys("alt-right")
    mode, focus, rects = front(clock)
    assert focus == clock and rects[fm] == (80, 1, 80, 48), (focus, rects)

    def mouse(*commands, lines):
        for command in commands:
            vm.hmp(command)
            time.sleep(.05)
        vm.serial(enter=False)
        wait(lines=lines)
        return state()

    def point(x, y):
        # From the cell the last state line names; its pixel within the cell stays the same.
        px, py = map(int, re.findall(r"POINTER=(\d+),(\d+)", "".join(seen))[-1])
        for move in mouse_moves((x - px) * 8, (y - py) * 16):
            vm.hmp(move)
            time.sleep(.05)

    point(81 + 5, 2 + 3)  # fm's left panel lists docs: "..", then notes.txt
    mode, focus, rects = mouse("mouse_button 1", "mouse_button 0", lines=2)
    assert focus == fm, (focus, rects)
    until(f"[WM] POINTER {fm} AT 5,3 BUTTONS=1 WHEEL=0")
    point(81 + 5, 2 + 2)
    mouse("mouse_button 1", "mouse_button 0", "mouse_button 1", "mouse_button 0", lines=4)
    mouse("mouse_move 0 0 -1", lines=1)
    until(f"[WM] POINTER {fm} AT 5,2 BUTTONS=0 WHEEL=1")
    vm.background(wm_pid)
    fm_log = vm.command(f"logs {started['fm']}")
    for line in ("[FM] POINTER 5,3 BUTTONS=1 WHEEL=0", "CURRENT=notes.txt", "LEFT=/ FULL", "[FM] POINTER 5,2 BUTTONS=0 WHEEL=1"):
        require(fm_log, line)
    vm.send(f"fg {wm_pid}\n")
    vm.expect(f"FOREGROUND PID={wm_pid}")
    # Snapped windows give their frame back (issue u002): fm's [⇕] the one it had before Alt+M snapped it; the clock,
    # snapped to the left half by the drag above, its own size when its title is dragged off the edge.
    point(80 + 80 - 7, 1)
    mode, focus, rects = mouse("mouse_button 1", "mouse_button 0", lines=2)
    assert rects[fm] == (0, 20, 80, 24), rects
    assert rects[clock] == (0, 1, 80, 48), rects
    point(20, 1)
    mode, focus, rects = mouse("mouse_button 1", *mouse_moves(30 * 8, 10 * 16), "mouse_button 0", lines=2)
    assert focus == clock and rects[clock] == (40, 11, 42, 13), (focus, rects)
    until(f"[WM] PIXELS {clock} 320X176")  # and its content the size it had
    # Recording one window (issue u014): `record -w` typed in the run line gets a read-only lease of the window in front,
    # the clock's; a console program, it runs in a console, which passes the lease on. wm marks the clock's frame while
    # it records. The frames are the window's content at its size, 320 x 176, whatever else is on the screen.
    names = [{" ": "spc", "-": "minus", ":": "shift-semicolon", ".": "dot"}.get(c, c) for c in "record -w -t 2 ram:win.avi"]
    keys("alt-r", *names, "ret", text="STARTED console")
    lent = re.search(r"\[WM\] STARTED console PID (\d+) WITH [^\n]*a window to see[^\n]*\(WINDOW (\d+) TO SEE\)", "".join(seen))
    assert lent and int(lent[2]) == clock, "".join(seen)[-1500:]
    until(f"[WM] RECORDING {clock}")
    while not any(m[1] == lent[1] for m in windows_re.findall("".join(seen))):
        wait("[WM] WINDOW", lines=0)
    console = next(int(m[0]) for m in windows_re.findall("".join(seen)) if m[1] == lent[1])
    mode, focus, rects = front(clock)
    screen = screen_text(vm)
    vm.serial(enter=False)
    x, y, w, h = rects[clock]
    assert "REC" in screen[y][x:x + 9], screen[y]
    until(f"[WM] RECORDED {clock}")  # the mark goes 3 s after the recording's time
    mode, focus, rects = front(console)
    x, y, w, h = rects[console]
    for _ in range(20):  # what record printed, in the console's window (console logs only to the system log)
        time.sleep(.5)
        screen = screen_text(vm)
        vm.serial(enter=False)
        said = "".join(row[x + 1:x + w - 1] for row in screen[y + 1:y + h - 1]).replace(" ", "")
        if "(recordended)" in said:
            break
    assert "REC" not in screen[rects[clock][1]][rects[clock][0]:rects[clock][0] + 9]
    summary = re.search(r"ram:win\.avi:(\d+)FRAMES\((\d+)ENCODED,\d+ROWSOFBLOCKSCODED,\d+MSEACH\),320X176AT10/S,(\d+)BYTES", said)
    assert summary and int(summary[1]) == 20 and int(summary[2]) >= 2, said
    keys("alt-w", text=f"CLOSE {console}")
    until(f"GONE {console}")
    while console in state()[2]:
        wait()
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
    # A program that ends at once with a failure leaves its message on view (211-APP-0039): camera, to which wm lends
    # no camera, says so in a window of its own with its status, until a key.
    keys("alt-r", "c", "a", "m", "e", "r", "a", "ret", text="STARTED camera")
    camera_pid = re.findall(r"\[WM\] STARTED camera PID (\d+)", "".join(seen))[-1]
    while not any(m[1] == camera_pid for m in windows_re.findall("".join(seen))):
        wait("[WM] WINDOW", lines=0)
    ended = next(int(m[0]) for m in windows_re.findall("".join(seen)) if m[1] == camera_pid)
    for _ in range(20):
        time.sleep(.3)
        screen = screen_text(vm)
        vm.serial(enter=False)
        if any(canon("camera: no camera was granted") in row for row in screen):
            break
    else:
        raise AssertionError(screen)
    time.sleep(1)  # it stays
    screen = screen_text(vm)
    vm.serial(enter=False)
    assert any(canon("camera: no camera was granted") in row for row in screen), screen
    assert any(canon("ENDED (STATUS 1): PRESS A KEY") in row for row in screen), screen
    assert any(canon("camera ended") in row for row in screen), "the window's title"
    assert ended in state()[2] and state()[1] == ended, state()
    keys("spc")
    until(f"GONE {ended}")
    while ended in state()[2]:
        wait()
    full_screen_and_list(fm, clock, top)
    # Leaving: the programs keep running; the next wm shows them where they were.
    places = state()[2]
    assert set(places) == {fm, clock, top}, places
    vm.hmp("sendkey alt-q"); vm.serial(enter=False)
    require(wait("RESUMED.", lines=0).replace("\n", ""), "DETACHED: 3 WINDOWS KEPT")  # the shell's mirror may break a line
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
    require(wait("RESUMED.", lines=0).replace("\n", ""), "CLOSE ALL: 3 WINDOWS")
    time.sleep(1); vm.collect(); vm.output = ""
    assert task_rows(vm) == {}, task_rows(vm)
    for _ in range(20):
        if heap_used(vm) == baseline:
            break
        time.sleep(.2)
    assert heap_used(vm) == baseline
    print("PASS: wm: fm, clock and top in windows (text frames and content, the clock's pixels, drawn again at its frame's size); keys to the window in front only; "
          "halves, quarters, maximize, Alt+M and snapping, a title dragged with the mouse, clicks, a double click and the wheel "
          "in fm's window, [⇕] and a snapped title dragged off the edge give the frame back; record -w records the clock's window "
          f"alone ({summary[1]} frames, {summary[2]} coded, 320x176) with REC on its frame; programs get only what wm holds; "
          "a program that ended at once with a failure leaves its message and status in its window until a key; "
          "leaving and a killed wm keep the programs and the next wm restores the places; close all ends them", flush=True)


def usb_suite(vm):
    """USB keyboards and pointers (issue 164): usb_host finds a keyboard behind a hub and a tablet on a root port (and a
    mouse plugged in later), usb_hid turns their reports into keys and pointer events; devices come and go at run time,
    and both drivers restart."""
    names = vm.services()
    assert "usb_host" in names and "usb_hid" in names and "ps2_kbd" not in names and "virtio_input" not in names, names
    hid = vm.service_logs("usb_hid", "TABLET")
    require(hid, "[USB_HID] 0627:0001 KEYBOARD")
    host = vm.command("dmesg -s usb_host", raw=True)
    hub = re.search(r"\[USB\] 0409:55AA ON PORT (\d+) \(FULL SPEED\) HUB", host)
    assert hub, host
    require(host, f"[USB] 0627:0001 ON PORT {hub[1]}.1 (FULL SPEED)")  # the keyboard on the hub's first port
    # The shell's keyboard client is usb_hid's (no PS/2 controller): keymap works through it.
    require(vm.command("keymap"), "LAYOUT: US  SWITCH: CTRL+SHIFT OR ALT+SHIFT  LAYOUTS: US RU")

    def typed(keys, hold=None):
        # The keys program's lines for keys sent to the USB keyboard.
        vm.send("run keys\n")
        vm.expect("[KEYS] READY")
        time.sleep(.2)
        start = len(vm.log)
        for key in keys:
            vm.hmp(f"sendkey {key}" + (f" {hold}" if hold else ""))
            time.sleep(.05 + (hold or 0) / 1000)  # sendkey returns at once; the key is released `hold` ms later
        vm.serial(enter=False)
        time.sleep(.5)
        vm.collect()
        got = re.findall(r"\[KEYS\] (code=[^\r\n]*)", vm.log[start:])
        vm.send_bytes(b"\x1b")
        vm.expect("EXITED. SHELL RESUMED.")
        time.sleep(.1); vm.collect(); vm.output = ""
        return got

    def in_order(got, expected):
        at = 0
        for line in expected:
            while at < len(got) and line not in got[at]:
                at += 1
            assert at < len(got), (line, got)
            at += 1

    keys = [("up", "code=Up mods=-"), ("shift-right", "code=Right mods=S"), ("f1", "code=F(1) mods=-"), ("f10", "code=F(10) mods=-"),
            ("delete", "code=Delete mods=-"), ("home", "code=Home mods=-"), ("pgdn", "code=PageDown mods=-"),
            ("ctrl-c", "code=Char mods=C char=c"), ("alt-x", "code=Char mods=A char=x"), ("shift-a", "code=Char mods=S char=A"),
            ("backspace", "code=Backspace mods=-"), ("tab", "code=Tab mods=-"),
            ("ctrl-shift", None), ("q", "char=й U+0439"), ("shift-q", "char=Й U+0419"), ("alt-shift", None), ("q", "char=q U+0071")]
    in_order(typed([k for k, _ in keys]), [line for _, line in keys if line])
    # A held key repeats: the host does it for a USB keyboard (after 500 ms, every 33 ms).
    got = typed(["w"], hold=1200)
    assert len([g for g in got if "char=w " in g]) >= 10, got
    # The tablet: a click on fm's first entry puts the cursor there.
    vm.send("fm\n")
    vm.expect("[FM] READY")
    time.sleep(.3)
    start = len(vm.log)
    vm.tablet_at(10 * 8 + 4, 2 * 16 + 8)
    time.sleep(.05)
    vm.tablet_click()
    # The cell where the pixel is depends on the screen's size: x86's is the 1280 x 800 the clicks assume.
    require(logged(vm, start, "BUTTONS=1"), "[FM] POINTER 10,2 BUTTONS=1 WHEEL=0" if vm.arch == "x86_64" else " BUTTONS=1 WHEEL=0")
    vm.send_bytes(b"\x1b"); time.sleep(.3); vm.send("\n"); time.sleep(.3); vm.collect(); vm.output = ""
    # A boot mouse plugged in at run time (211-DRV-0003): its layout from the descriptor in the report protocol, its
    # first reports logged, its movement to the focused program.
    vm.qmp("device_add", driver="usb-mouse", bus="xhci.0", port="1.3", id="mouse")
    require(vm.service_logs("usb_hid", "MOUSE"), "[USB_HID] 0627:0001 MOUSE: ID 0, X AT BIT 8 (8 BITS), Y AT BIT 16 (8 BITS), REPORT PROTOCOL")
    vm.send("run keys\n")
    vm.expect("[KEYS] READY")
    time.sleep(.3)
    start = len(vm.log)
    vm.hmp("mouse_move 20 10")
    time.sleep(.5)
    vm.collect()
    moves = [tuple(map(int, m)) for m in re.findall(r"\[KEYS\] pointer buttons=0 dx=(-?\d+) dy=(-?\d+) wheel=0", vm.log[start:])]
    vm.send_bytes(b"\x1b")
    vm.expect("EXITED. SHELL RESUMED.")
    time.sleep(.1); vm.collect(); vm.output = ""
    assert sum(m[0] for m in moves) == 20 and sum(m[1] for m in moves) == 10, moves
    require(vm.service_logs("usb_hid", "REPORT ["), "[USB_HID] 0627:0001 REPORT [00, ")
    # Unplugged and plugged in again, on another hub port: usb_hid lets it go and takes the new one.
    vm.qmp("device_del", id="kbd")
    require(vm.service_logs("usb_hid", "DEVICE GONE"), "[USB_HID] DEVICE GONE")
    vm.qmp("device_add", driver="usb-kbd", bus="xhci.0", port="1.2", id="kbd2")
    require(vm.service_logs("usb_hid", "KEYBOARD"), "[USB_HID] 0627:0001 KEYBOARD")
    in_order(typed(["m", "n"]), ["char=m U+006D", "char=n U+006E"])
    # Both drivers restart; the keyboard works again (usb_hid claims it from the new usb_host).
    for driver in ("usb_hid", "usb_host"):
        pid = vm.services()[driver]
        require(vm.command(f"kill {pid}", raw=True), f"KILLED PID={pid}")
        require(vm.service_logs("init", f"{driver} RESTARTED"), f"{driver} RESTARTED")
        require(vm.service_logs("usb_hid", "KEYBOARD"), "[USB_HID] 0627:0001 KEYBOARD")
        in_order(typed(["z"]), ["char=z U+007A"])
    print("PASS: USB keyboard behind a hub, tablet and mouse: keys, layouts, repeat, a click, movement, hot plug, driver restarts", flush=True)


def tablet_suite(vm, wav):
    """The VirtIO tablet (issue 161): the host's pointer as a position, so the system's pointer is where the host's is
    and reaches the edges of the screen — fm's key bar in the bottom right corner, wm's buttons at the right edge."""
    require(vm.service_logs("virtio_input", "[VIRTIO_INPUT] "), "[VIRTIO_INPUT] QEMU Virtio Tablet X=0..32767 Y=0..32767")

    def click(x, y, text, button="left"):
        # A click on cell (x, y); the log once `text` came.
        start = len(vm.log)
        vm.tablet_at(x * 8 + 4, y * 16 + 8)
        time.sleep(.05)
        vm.tablet_click(button)
        return logged(vm, start, text)

    # fm on its own screen: a click puts the cursor on the entry under the host's pointer; 10 Quit in the corner ends it.
    vm.send("fm\n")
    vm.expect("[FM] READY")
    time.sleep(.3)
    require(click(10, 2, "CURRENT=EFI "), "[FM] POINTER 10,2 BUTTONS=1 WHEEL=0")  # the first entry
    require(click(159, 49, "[FM] DONE"), "[FM] POINTER 159,49 BUTTONS=1 WHEEL=0")
    vm.expect("SHELL RESUMED.")
    # Quit in a program's menu and key bar (issue u013): edit's File > Quit clicked (F9 opens the menu: File's third
    # item is on row 4); view's 10 Quit clicked.
    vm.send("edit ram:quit.txt\n")
    vm.expect("[EDIT] READY")
    time.sleep(.3)
    vm.hmp("sendkey f9")
    vm.serial(enter=False)
    time.sleep(.3)
    require(click(5, 4, "[EDIT] DONE"), "[EDIT] POINTER 5,4 BUTTONS=1 WHEEL=0")
    vm.expect("SHELL RESUMED.")
    vm.send("view kernel.elf\n")
    vm.expect("[VIEW] OPEN")
    time.sleep(.3)
    click(159, 49, "[VIEW] DONE")
    vm.expect("SHELL RESUMED.")
    # wm: top in the top right quarter; its [▲] maximizes it, [×] next to the screen's right edge closes it.
    start = len(vm.log)
    vm.send("wm fm, top\n")
    out = logged(vm, start, "[WM] READY")
    for _ in range(100):
        out = logged(vm, start, "[WM] READY")
        if len(re.findall(r"\[WM\] WINDOW \d+ PID", out)) >= 2:
            break
        time.sleep(.1)
    top_pid = re.search(r"\[WM\] STARTED top PID (\d+)", out)[1]
    top = int(re.search(fr"\[WM\] WINDOW (\d+) PID {top_pid} ", out)[1])
    time.sleep(1)
    out = click(153, 1, f"{top}@0,1,160x48")
    assert re.search(fr"FOCUS={top} .*POINTER=153,1", out), out[-600:]
    start = len(vm.log)
    require(click(156, 1, f"[WM] CLOSE {top}"), f"[WM] CLOSE {top}")
    logged(vm, start, f"[WM] GONE {top}", timeout=12)  # top ends: its frame no longer covers the desktop
    assert "POINTER=159,49" in click(159, 49, "POINTER=159,49"), "the bottom right corner"
    # The desktop menu (issue u003): a right click on the desktop lists the programs by category; the mouse on Clocks
    # opens its programs beside it (the categories are 20 cells wide: "Sound and voice"); a click starts clock.
    logged(vm, 0, "[WM] PROGRAMS: ")
    assert "MODE=MENU" in click(100, 35, "MODE=MENU", button="right")
    vm.tablet_at(103 * 8 + 4, 38 * 16 + 8)
    time.sleep(.2)
    require(click(121, 38, "[WM] STARTED clock PID"), "[WM] STARTED clock PID")
    # The top bar's items can be clicked (issue u008): help opens and a click closes it; the run line opens.
    assert "MODE=HELP" in click(80, 0, "MODE=HELP")
    assert "MODE=NORMAL" in click(80, 30, "MODE=NORMAL")
    assert "MODE=RUN" in click(40, 0, "MODE=RUN")
    # A console program started in wm runs in a window of console, which shows what it prints (issue u004).
    start = len(vm.log)
    for key in ("u", "p", "t", "i", "m", "e", "ret"):
        vm.hmp(f"sendkey {key}")
        time.sleep(.08)
    vm.serial(enter=False)
    require(logged(vm, start, "[WM] STARTED console PID"), "[WM] STARTED console PID")
    for _ in range(30):
        time.sleep(.3)
        screen = screen_text(vm)
        vm.serial(enter=False)
        if any(canon("> uptime") in row for row in screen) and any(canon("║up 0:") in row for row in screen):
            break
    else:
        raise AssertionError(screen)
    # beep from the desktop menu (issue u011): Sound and voice > beep runs in a console window of its own, in the
    # bottom right quarter, which shows beep's lines; its tones reach the sound card (checked in the WAV below).
    assert "MODE=MENU" in click(100, 35, "MODE=MENU", button="right")
    vm.tablet_at(103 * 8 + 4, 39 * 16 + 8)
    time.sleep(.2)
    require(click(121, 39, "[WM] STARTED console PID"), "[WM] STARTED console PID")
    for _ in range(30):
        time.sleep(.3)
        screen = screen_text(vm)
        vm.serial(enter=False)
        if any(canon("[BEEP] DONE") in row[80:] for row in screen[25:]):
            break
    else:
        raise AssertionError(screen)
    assert any(canon("[BEEP] DEVICE=true RATE=48000") in row[80:] for row in screen[25:]), screen
    # A window held by its title is marked until the button is released (211-APP-0037): beep's window is pressed on its
    # title through the tablet, which marks its frame in the accent colour; moved and released, it is unmarked.
    beep_pid = re.findall(r"\[WM\] STARTED console PID (\d+)", logged(vm, 0, "[WM] STARTED console PID"))[-1]
    beep = int(re.search(fr"\[WM\] WINDOW (\d+) PID {beep_pid} ", logged(vm, 0, f"PID {beep_pid} "))[1])

    def state(out):
        # beep's frame (cells) and whether wm holds it, from the last state line in `out`.
        line = re.findall(r"\[WM\] MODE=.*$", out, re.M)[-1]
        return tuple(map(int, re.search(fr"\b{beep}@(\d+),(\d+),(\d+)x(\d+)", line).groups())), f" DRAG={beep}" in line

    def accent_on_left_edge(frame):
        # Pixels in wm's accent colour (DARK) on the left edge of `frame`, below its title.
        _, size, _, pixels = vm.screenshot().split(b"\n", 3)
        width, height = map(int, size.split())
        x0, y0 = width % 8 // 2, height % 16 // 2
        x, y, _, h = frame
        return sum(pixels[(py * width + px) * 3:(py * width + px) * 3 + 3] == b"\xa6\xe3\xa1"
                   for py in range(y0 + (y + 1) * 16, y0 + (y + h - 1) * 16) for px in range(x0 + x * 8, x0 + x * 8 + 8))

    (x, y, w, h), held = state(logged(vm, 0, "[WM] MODE="))
    assert not held and accent_on_left_edge((x, y, w, h)) == 0, (x, y, w, h)
    start = len(vm.log)
    vm.tablet_at((x + w // 3) * 8 + 4, y * 16 + 8)
    time.sleep(.05)
    vm.qmp("input-send-event", events=[{"type": "btn", "data": {"down": True, "button": "left"}}])
    pressed, held = state(logged(vm, start, f" DRAG={beep}"))
    assert held and pressed == (x, y, w, h), (pressed, held)
    time.sleep(.3)
    assert accent_on_left_edge(pressed) >= 8 * (h - 2), "the held window's frame in the accent colour"
    # Moves while the button is held are not logged; the release is, with where the window went.
    to = (x + w // 3 - 20, y - 6)
    vm.tablet_at(to[0] * 8 + 4, to[1] * 16 + 8)
    time.sleep(.3)
    start = len(vm.log)
    vm.qmp("input-send-event", events=[{"type": "btn", "data": {"down": False, "button": "left"}}])
    dropped, held = state(logged(vm, start, f"POINTER={to[0]},{to[1]}"))
    assert not held and dropped[:2] != (x, y), (dropped, held)
    time.sleep(.3)
    assert accent_on_left_edge(dropped) == 0, "no mark after the release"
    start = len(vm.log)
    vm.hmp("sendkey alt-x"); vm.serial(enter=False)
    require(logged(vm, start, "RESUMED.", timeout=12).replace("\n", ""), "CLOSE ALL: 4 WINDOWS")
    time.sleep(1); vm.collect(); vm.output = ""
    assert task_rows(vm) == {}, task_rows(vm)
    vm.close()
    import struct, wave
    with wave.open(str(wav)) as audio:
        frames = audio.readframes(audio.getnframes())
        rate = audio.getframerate()
    left = struct.unpack(f"<{len(frames) // 2}h", frames)[0::2]
    loud = [i for i, sample in enumerate(left) if sample]
    assert loud, "beep from the menu: no sound"
    beep_demo_tones(left, rate, loud[0])
    print("PASS: tablet: the VirtIO tablet's positions; fm clicked through it, 10 Quit in the bottom right corner; edit's File > Quit and view's 10 Quit clicked; wm's [▲] and [×] at the screen's right edge; "
          "the desktop menu opened by a right click, a program started from its Clocks submenu; the top bar clicked (help, run); uptime in a console window; "
          "beep from the menu: its lines in its console window, its tones in the WAV; a window dragged by its title marked until the release", flush=True)


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
    vm.program_logs(holder, "MANAGER LEAVES", 40)
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


def store_disk_check(args, disk):
    """300-KRN-0025: a blank VirtIO disk is the block store's own: the store formats it, vfs_server never gets it,
    and an object put there is found after a reboot (the snapshot overlay outlives the guest's reboot)."""
    with tempfile.TemporaryDirectory(prefix="mind-store-") as temp:
        blank = Path(temp) / "store.img"
        with blank.open("wb") as f:
            f.truncate(32 << 20)
        extra = ("-drive", f"format=raw,file={blank},if=none,id=store", "-device", "virtio-blk-pci,drive=store")
        vm = VM(args, disk.relative_to(ROOT).as_posix(), reboot=True, extra=extra)
        try:
            require(vm.command("dmesg -s init", raw=True), "[INIT] THE BLOCK STORE'S DISK: virtio_blk (BLANK)")
            assert "FROM VIRTIO" not in vm.command("dmesg -s vfs_server", raw=True)
            put = re.search(r"PUT (\S+) SIZE 5000", vm.command("blocks pattern 5000", raw=True))
            assert put, vm.output
            require(vm.command("blocks stat", raw=True), "SECTORS=")
            vm.send("reboot\n")
            vm.expect("MIND CORE KERNEL: INIT STARTED", timeout=90)
            vm.expect("MIND>", timeout=60)
            time.sleep(1); vm.collect(); vm.output = ""
            require(vm.command("dmesg -s init", raw=True), "[INIT] THE BLOCK STORE'S DISK: virtio_blk (THE STORE'S)")
            stat = vm.command("blocks stat", raw=True)
            assert re.search(r"BLOCKS=[1-9]\d* .*SECTORS=\d+/65536", stat), stat
            require(vm.command(f"blocks check {put[1]} pattern", raw=True), "CHECKED 5000 BYTES = PATTERN")
        finally:
            vm.close()
    print("PASS: a blank VirtIO disk becomes the block store's own (not vfs_server's), and an object put there is found after a reboot", flush=True)


def efivar_check(args, disk):
    """351-KRN-0027 (x86, OVMF), 351-KRN-0028 (aarch64, AAVMF): efivar reads the firmware's boot variables through the
    kernel's UEFI runtime services, only after the user allows it; BootNext set to the firmware's own shell boots that
    shell once, and the boot after it is MIND Core again, with BootNext consumed by the firmware."""
    vm = VM(args, disk.relative_to(ROOT).as_posix(), reboot=True)

    def efivar(line, answer):
        vm.send(line + "\n")
        vm.expect("ASKS TO READ AND CHANGE THE FIRMWARE'S BOOT SETTINGS. ALLOW? (Y/N)", timeout=20)
        vm.send_bytes(answer)
        return vm.expect("MIND> ", timeout=20)

    try:
        require(efivar("efivar", b"n"), "efivar: the firmware's variables were not granted")
        listing = efivar("efivar", b"y")
        current = re.search(r"BootCurrent: ([0-9A-F]{4})", listing)
        shell = re.search(r"Boot([0-9A-F]{4}) EFI Internal Shell", listing)
        assert current and shell and "BootNext: not set" in listing and "BootOrder:" in listing, listing
        require(efivar(f"efivar bootnext {shell[1]}", b"y"), f"BootNext SET TO {shell[1]}")
        vm.send("reboot\n")
        vm.expect("Shell>", timeout=90)
        vm.send_bytes(b"reset\r")
        vm.expect("MIND CORE KERNEL: INIT STARTED", timeout=90)
        vm.expect("MIND>", timeout=60)
        time.sleep(1); vm.collect(); vm.output = ""
        listing = efivar("efivar", b"y")
        assert f"BootCurrent: {current[1]}" in listing and "BootNext: not set" in listing, listing
    finally:
        vm.close()
    print(f"PASS: efivar lists the boot entries after the user allows it (refused without); BootNext {shell[1]} boots the "
          f"firmware's shell once, then Boot{current[1]} again with BootNext consumed", flush=True)


def devicetree_suite(args, disk):
    """210-KRN-0029, for 210-APL-0002: QEMU virt without ACPI, where the firmware hands over a device tree instead; the
    bootloader passes its address in BootInfo and the kernel checks its header. The kernel has no console there until
    it reads the board from the tree, and init's services soon write over its lines on the screen, so the machine is
    stopped once the bootloader names the tree and run on in 10 ms steps until the kernel's line is on the screen."""
    if args.arch != "aarch64":
        print("SKIP: devicetree: OVMF on x86 hands over no device tree", flush=True)
        return
    machine = (args.machine or "virt,gic-version=3,highmem=off") + ",acpi=off"
    vm = VM(argparse.Namespace(**{**vars(args), "machine": machine}), disk.relative_to(ROOT).as_posix(), prompt=False)
    try:
        deadline = time.monotonic() + 90
        while not (loader := re.search(r"BOOT: DEVICE TREE AT (0x[0-9a-f]+), (\d+) BYTES", vm.log)) and time.monotonic() < deadline and vm.process.poll() is None:
            time.sleep(0.02)
            vm.collect()
        vm.hmp("stop")
        assert loader, vm.log[-3000:]
        kernel = None
        for _ in range(500):
            kernel = next((m for line in screen_text(vm) if (m := re.search(r"MIND CORE KERNEL: DEVICE TREE AT (0x[0-9a-f]+), (\d+) BYTES, VERSION (\d+)", line))), None)
            if kernel:
                break
            vm.hmp("cont")
            time.sleep(0.01)
            vm.hmp("stop")
        assert kernel and kernel[1] == loader[1] and kernel[2] == loader[2] and int(kernel[3]) >= 16, (loader, kernel)
    finally:
        vm.close()
    print(f"PASS: devicetree: on virt without ACPI the bootloader passes the device tree at {loader[1]} ({loader[2]} bytes) "
          f"and the kernel finds an FDT header there (version {kernel[3]})", flush=True)


def bar_move_check(args, disk):
    """211-KRN-0021, with a kernel that packs the RTL8139's 256-byte register BAR into the SD host controller's page, as
    Apple's firmware packs EHCI next to AHCI: granted, the BAR moves to a free page of its own, and the card's MAC
    address reads there."""
    with tempfile.TemporaryDirectory(prefix="mind-bar-") as temp:
        volume = Path(temp) / "volume"
        shutil.copytree(disk, volume, ignore=shutil.ignore_patterns("smoke-*"))
        (volume / "kernel.elf").write_bytes(Path(args.bar_kernel).read_bytes())
        sign_manifest.sign_volume(volume)
        vm = VM(args, str(volume), prompt=False, extra=["-device", "sdhci-pci", "-netdev", "user,id=n9", "-device", "rtl8139,netdev=n9,mac=52:54:00:12:34:58"])
        try:
            out = vm.expect("READ THERE", timeout=60)
            packed = re.search(r"PCI TEST: BAR 1 OF (\w+) PACKED AT ([0-9A-F]+), IN THE PAGE OF (\w+)", out)
            moved = re.search(r"PCI: BAR 1 OF (\w+) MOVED FROM ([0-9A-F]+) TO ([0-9A-F]+): ITS PAGE HELD REGISTERS OF (\w+)", out)
            granted = re.search(r"PCI TEST: GRANTED AT ([0-9A-F]+), MAC ([0-9A-F:]+) READ THERE", out)
            assert packed and moved and granted, out[-3000:]
            assert moved[2] == packed[2] and moved[4] == packed[3] and int(moved[3], 16) % 4096 == 0 and int(moved[3], 16) >> 12 != int(packed[2], 16) >> 12, out[-3000:]
            assert granted[1] == moved[3] and granted[2] == "52:54:00:12:34:58", out[-3000:]
        finally:
            vm.close()
    print(f"PASS: a BAR in another kind of device's page moves to a page of its own when granted ({packed[2]} to {moved[3]}), and the device answers there", flush=True)


def trial_check(args, disk):
    """351-KRN-0014, with a kernel whose trial deadline is 15 s: slot B booted on trial from a disk MIND Core drives is
    confirmed by init and stays up past the deadline; booted from one it has no driver for (USB on EHCI, which
    usb_host does not take without xHCI), init finds no boot volume and does not confirm, the kernel restarts the
    machine at the deadline, and the next boot falls back to slot A."""
    with tempfile.TemporaryDirectory(prefix="mind-trial-") as temp:
        temp = Path(temp)
        volume = temp / "volume"
        shutil.copytree(disk, volume, ignore=shutil.ignore_patterns("smoke-*"))
        (volume / "kernel.elf").write_bytes(Path(args.trial_kernel).read_bytes())
        sign_manifest.sign_volume(volume)
        image = boot_slots.Image.create(temp / "trial.img", boot_slots.layout(volume, temp / "slots", both=True))
        shutil.rmtree(volume)
        shutil.rmtree(temp / "slots")
        boot_slots.write_next(image, slot="B", fallback="A", tries=1)
        vm = VM(args, str(image.path), raw=True, prompt=False)
        try:
            out = vm.expect("MIND CORE KERNEL: THE TRIAL BOOT IS CONFIRMED", timeout=90)
            for line in ("BOOT: SLOT B LOADED ON TRIAL", "MIND CORE KERNEL: SLOT B ON TRIAL: A RESTART IN 15 S UNLESS INIT CONFIRMS IT"):
                require(out, line)
            # init's lines go to the system log, not COM1; logs 1 is the shell's own command (init is PID 1).
            require(vm.command("logs 1", raw=True), "[INIT] TRIAL BOOT CONFIRMED: EVERY BOOT SERVICE STARTED, THE BOOT VOLUME MOUNTED")
            time.sleep(18)
            vm.collect()
            assert vm.process.poll() is None and "RESTARTING" not in vm.log, vm.log[-2000:]
        finally:
            vm.close()
        boot_slots.write_next(image, slot="B", fallback="A", tries=1)
        with tempfile.TemporaryDirectory(prefix="smoke-empty-", dir=ROOT / IMAGE) as empty:
            ehci = ("-drive", f"format=raw,file={image.path},if=none,id=trial", "-device", "usb-ehci,id=ehci",
                    "-device", "usb-storage,bus=ehci.0,drive=trial,bootindex=0")
            vm = VM(args, Path(empty).relative_to(ROOT).as_posix(), reboot=True, extra=ehci)
            try:
                require(ANSI.sub("", vm.log), "BOOT: SLOT B LOADED ON TRIAL")
                # init's verdict may come after the shell's prompt; `logs` drains, so the readings add up.
                verdict = ""
                for _ in range(60):
                    verdict += vm.command("logs 1", raw=True)
                    if "[INIT] TRIAL BOOT" in verdict:
                        break
                    time.sleep(.25)
                require(verdict, "[INIT] TRIAL BOOT NOT CONFIRMED: NO BOOT VOLUME MOUNTED")
                vm.expect("MIND CORE KERNEL: THE TRIAL BOOT WAS NOT CONFIRMED IN 15 S: RESTARTING", timeout=60)
                out = vm.expect("BOOT: SLOT A LOADED", timeout=90)
                require(out, "BOOT: SLOT B NOT CONFIRMED, NO TRIES LEFT")
            finally:
                vm.close()
    print("PASS: a trial boot that init confirms stays up past its deadline; one it cannot confirm (no boot volume) "
          "restarts at the deadline, and the next boot falls back to slot A", flush=True)


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
        # Signed again, so the bootloader gets past the manifest to the ELF it checks; a missing service is not, so the
        # manifest still lists it (one it does not list may be absent, 351-KRN-0022).
        if data is not None:
            sign_manifest.sign_volume(disk)
        vm = VM(args, disk.relative_to(ROOT).as_posix(), prompt=False)
        try:
            vm.expect(f"BOOT ERROR: {name}: {reason}", timeout=30)
        finally:
            vm.close()
        target.write_bytes(original)
    sign_manifest.sign_volume(disk)
    print("PASS: bootloader reports a corrupt kernel ELF (magic, truncated header, program headers) and a missing boot file the manifest lists", flush=True)
    # 350-UPD-0003: nothing is loaded that the signed manifest does not describe. A changed image, a changed manifest,
    # a manifest signed by another key or no signature: the bootloader names the file and stops.
    manifest, rtc = (disk / "MANIFEST").read_bytes(), (disk / "rtc.elf").read_bytes()
    cases = [("kernel.elf", lambda: (disk / "kernel.elf").write_bytes(kernel[:-1] + bytes([kernel[-1] ^ 1])), "not as the manifest says"),
             ("rtc.elf", lambda: (disk / "rtc.elf").write_bytes(rtc + b"\0"), "not as the manifest says"),
             ("MANIFEST", lambda: (disk / "MANIFEST").write_bytes(manifest.replace(b"toolchain ", b"toolchain-")), "bad signature"),
             ("MANIFEST", lambda: sign_manifest.sign_volume(disk, sign_manifest.hashlib.sha256(b"not the key").digest()), "bad signature"),
             ("MANIFEST.SIG", lambda: (disk / "MANIFEST.SIG").unlink(), "file not found")]
    for name, change, reason in cases:
        change()
        vm = VM(args, disk.relative_to(ROOT).as_posix(), prompt=False)
        try:
            vm.expect(f"BOOT ERROR: {name}: {reason}", timeout=30)
        finally:
            vm.close()
        (disk / "kernel.elf").write_bytes(kernel)
        (disk / "rtc.elf").write_bytes(rtc)
        sign_manifest.sign_volume(disk)
    print("PASS: signed boot volume: a changed kernel or service, a changed manifest, another key's signature and no "
          "signature each stop the bootloader before anything is loaded", flush=True)
    # 211-KRN-0012: the firmware lists another disk's EFI partition first (a Mac's internal disk); the loader reads the
    # kernel and the services from its own volume.
    # The decoy is on IDE, served by ata, the first block driver; the boot disk on AHCI. vfs_server mounts the volume
    # the bootloader names in BootInfo, holding the manifest it verified, not the first one it sees (211-KRN-0012).
    with tempfile.TemporaryDirectory(prefix="smoke-decoy-", dir=ROOT / IMAGE) as decoy:
        (Path(decoy) / "EFI/APPLE").mkdir(parents=True)
        vm = VM(args, disk.relative_to(ROOT).as_posix(), ahci=True, decoy=Path(decoy).relative_to(ROOT).as_posix())
        try:
            require(ANSI.sub("", vm.log), "BOOT: VOLUME MBR PARTITION 1 AT LBA 63")
            vfs = vm.command("dmesg -s vfs_server", raw=True)
            require(vfs, "[VFS] MOUNTED FAT16 FROM AHCI AT LBA 63")
            require(vfs, "[VFS] THE BOOT VOLUME: MBR DISK BE1AFDFA, PARTITION 1 AT LBA 63, AND THE MANIFEST THE BOOTLOADER VERIFIED")
            require(vm.command("ls"), "kernel.elf")
        finally:
            vm.close()
    print("PASS: the bootloader reads its own volume when the firmware lists another disk's FAT volume first, and names it "
          "in BootInfo: vfs_server mounts that one (AHCI), not the other disk ahead of it (IDE)", flush=True)
    # 350-UPD-0004: the launch record the bootloader printed on COM1 is in the running system's log, the same.
    vm = VM(args, disk.relative_to(ROOT).as_posix())
    try:
        serial = re.search(r"BOOT: MANIFEST (\S+ KEY \S+(?: \(THE TEST KEY\))? VERIFIED, \d+ IMAGES CHECKED)", ANSI.sub("", vm.log))
        assert serial, vm.log[-3000:]
        require(vm.command("dmesg -s init", raw=True), f"[INIT] LAUNCH: MANIFEST {serial[1]}; THE VOLUME'S ROOT")
    finally:
        vm.close()
    print("PASS: the launch record (manifest, key, images checked) in the system log matches the bootloader's serial line", flush=True)
    # 211-KRN-0016: two GPUs, the first listed without a linear framebuffer (virtio-gpu): the loader takes the GOP of a
    # console output that has one. Its progress lines name each step on the console (COM1 here, through the firmware).
    vm = VM(args, disk.relative_to(ROOT).as_posix(), extra=["-vga", "none", "-device", "virtio-gpu-pci", "-device", "VGA"])
    try:
        log = ANSI.sub("", vm.log).replace("\r", "")
        for line in (r"MIND CORE BOOT: STARTED; READING THE KERNEL AND THE SERVICES FROM ITS OWN VOLUME\n",
                     r"MIND CORE BOOT: KERNEL AND \d+ SERVICES READ\n", r"MIND CORE BOOT: \d+ GRAPHICS OUTPUTS\n",
                     r"MIND CORE BOOT: GOP 0: \d+x\d+ STRIDE \d+ BLT ONLY FB 0x0+ CONSOLE\n",
                     r"MIND CORE BOOT: USING GOP 1: \d+x\d+ STRIDE \d+ BGR FB 0x[0-9A-F]{16}\n", r"MIND CORE BOOT: \d+ CPUS; EXITING BOOT SERVICES\n"):
            assert re.search(line, log), (line, log[-3000:])
    finally:
        vm.close()
    print("PASS: bootloader takes the GOP of a console output with a linear framebuffer, not the first listed; its progress lines show on the console", flush=True)
    # 351-UPD-0006: slots A and B on a raw disk, where the bootloader counts a trial's tries and falls back.
    def boot_image(image, until):
        vm = VM(args, str(image), raw=True, snapshot=False, prompt=False)
        try:
            return vm.expect(until, timeout=60)
        finally:
            vm.close()
    with tempfile.TemporaryDirectory(prefix="mind-slots-") as temp:
        boot_slots_check.run(boot_image, temp, disk, "x86")
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
        sign_manifest.sign_volume(disk)
        vm = VM(args, disk.relative_to(ROOT).as_posix(), prompt=False)
        try:
            deadline, pattern = time.monotonic() + 30, re.compile(r"KERNEL PANIC: panic test at src/scheduler\.rs:\d+:\d+ CPU=\d+ PID=\d+ NAME=init\n")
            while not pattern.search(vm.output.replace("\r", "")):
                assert time.monotonic() < deadline and vm.process.poll() is None, vm.output[-2000:]
                vm.collect(); time.sleep(.05)
            # 211-KRN-0013: the boot line and the report are on the screen too, for a machine without a serial port.
            screen = "\n".join(line.rstrip() for line in screen_text(vm))
            assert re.search(r"MIND CORE KERNEL: INIT STARTED\n(.*\n)*KERNEL PANIC: panic test at src/scheduler\.rs:\d+:\d+ CPU=\d+ PID=\d+ NAME=init", screen), screen
        finally:
            vm.close()
        target.write_bytes(kernel)
        sign_manifest.sign_volume(disk)
        print("PASS: kernel panic report names message, source location, CPU and running task, on COM1 and on the screen", flush=True)
    if args.abi_kernel:
        # Issue 172: programs built for another ABI version stop at once; init does (exit code 126), so the system halts.
        target = disk / "kernel.elf"
        target.write_bytes(Path(args.abi_kernel).read_bytes())
        sign_manifest.sign_volume(disk)
        vm = VM(args, disk.relative_to(ROOT).as_posix(), prompt=False)
        try:
            vm.expect("INIT EXITED: SYSTEM HALTED (REASON=0000000000007E00)", timeout=60)
            screen = "\n".join(screen_text(vm))
            assert "INIT EXITED: SYSTEM HALTED (REASON=0000000000007E00)" in screen, screen
        finally:
            vm.close()
        target.write_bytes(kernel)
        sign_manifest.sign_volume(disk)
        print("PASS: a kernel of another ABI version: init refuses to run (ABI MISMATCH, exit 126) and the system halts", flush=True)
    abi = int(re.search(r"pub const ABI_VERSION: u32 = (\d+);", (ROOT / "common/abi.rs").read_text())[1])
    if args.loader_abi_kernel:
        # 211-KRN-0012: a kernel and a bootloader of different ABI versions: the kernel stops at once and says why.
        target = disk / "kernel.elf"
        target.write_bytes(Path(args.loader_abi_kernel).read_bytes())
        sign_manifest.sign_volume(disk)
        vm = VM(args, disk.relative_to(ROOT).as_posix(), prompt=False)
        line = f"KERNEL STOPPED: THE BOOTLOADER IS OF ABI {abi}, THIS KERNEL OF ABI {abi + 1}. WRITE BOTH FROM ONE BUILD."
        try:
            out = vm.expect(line, timeout=60)
            assert "INIT STARTED" not in out, out[-2000:]
            screen = "\n".join(screen_text(vm))
            assert line in screen, screen
        finally:
            vm.close()
        target.write_bytes(kernel)
        sign_manifest.sign_volume(disk)
        print("PASS: a bootloader of another ABI version: the kernel stops before init, with the reason on COM1 and on the screen", flush=True)
    if args.trial_kernel:
        trial_check(args, disk)
    if args.bar_kernel:
        bar_move_check(args, disk)
    store_disk_check(args, disk)
    efivar_check(args, disk)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--arch", choices=("x86_64", "aarch64"), default="x86_64", help="aarch64: QEMU virt with the aarch64_root build (scripts/build_aarch64.sh)")
    parser.add_argument("--qemu", default=os.environ.get("QEMU"))
    parser.add_argument("--cpus", type=int)
    parser.add_argument("--machine", help="QEMU -machine; aarch64: in place of virt,gic-version=3,highmem=off (issue 205: highmem=on, gic-version=2); x86: added properties (211-PRT-0003: pit=off)")
    parser.add_argument("--memory", help="QEMU -m (default 512; e.g. 6G: RAM above 4 GiB)")
    parser.add_argument("--disk", choices=("default", "nvme"), default="default", help="the boot disk's bus: nvme puts it on an NVMe controller (issue 205)")
    parser.add_argument("--aavmf-code", default="/usr/share/AAVMF/AAVMF_CODE.fd")
    parser.add_argument("--aavmf-vars", default="/usr/share/AAVMF/AAVMF_VARS.fd")
    parser.add_argument("--cpu-model", help="QEMU -cpu model, e.g. max: AVX state saved with XSAVE (issue 153)")
    parser.add_argument("--firmware", default="OVMF.fd")
    parser.add_argument("--busy-elf", help="test-only ELF built from tests/busy_app.rs")
    parser.add_argument("--isolation-elf", help="test-only ELF built from tests/isolation_app.rs")
    parser.add_argument("--heap-elf", help="test-only ELF built from tests/heap_app.rs")
    parser.add_argument("--block-elf", help="test-only ELF built from tests/block_app.rs (stands in for vfs_server)")
    parser.add_argument("--updater-elf", help="test-only ELF built from tests/updater_stub (stands in for the updater, 351-KRN-0022)")
    parser.add_argument("--panic-kernel", help="test-only kernel built with --features panic-test (boot suite)")
    parser.add_argument("--abi-kernel", help="test-only kernel built with --features abi-test (boot suite, issue 172)")
    parser.add_argument("--loader-abi-kernel", help="test-only kernel built with --features loader-abi-test (boot suite, 211-KRN-0012)")
    parser.add_argument("--trial-kernel", help="test-only kernel built with --features trial-test (boot suite, 351-KRN-0014)")
    parser.add_argument("--bar-kernel", help="test-only kernel built with --features bar-move-test (boot suite, 211-KRN-0021)")
    parser.add_argument("--kernel", help="run the suites with this kernel, in a copy of the image directory (e.g. --features x2apic-test)")
    parser.add_argument("--suites", help="comma-separated subset: boot,display,net,tls,netbench,devicetree (aarch64),efivar,windows,wm,tablet,usb,normal,memory,dzen,services,store,storefaults,ahci,audio,tts,listen,keys,shell,tools,vfs,edit,disk,busy,smp,isolation,heap,block,updater,hda,ehci")
    parser.add_argument("--bench-mib", type=int, default=4, help="MiB moved each way by the netbench suite")
    parser.add_argument("--bench-runs", type=int, default=1, help="netbench runs per offload setting")
    parser.add_argument("--tap", help="netbench suite over this tap interface (host address 10.0.2.2/24) instead of user networking")
    parser.add_argument("--asr-model", help="optional Vosk model directory (Russian) to check that tts speech is recognizable")
    args = parser.parse_args()
    global IMAGE, BOOT_EFI, BOOT_DRIVE, BOOT_DRIVER
    if args.arch == "aarch64":
        # Four CPUs (issue 203); the busy fixture from scripts/build_aarch64.sh --fixtures.
        IMAGE, BOOT_EFI = "aarch64_root", "EFI/BOOT/BOOTAA64.EFI"
        BOOT_DRIVE, BOOT_DRIVER = "VIRTIO", "virtio_blk"
        args.qemu, args.cpus = args.qemu or "qemu-system-aarch64", args.cpus or 4
        fixture = ROOT / IMAGE / "fixture-busy_app.elf"
        args.busy_elf = args.busy_elf or (str(fixture) if fixture.exists() else None)
        fixture = ROOT / IMAGE / "fixture-updater.elf"
        args.updater_elf = args.updater_elf or (str(fixture) if fixture.exists() else None)
    args.qemu, args.cpus = args.qemu or "qemu-system-x86_64", args.cpus or 4
    if args.disk == "nvme":
        BOOT_DRIVE, BOOT_DRIVER = "NVME", "nvme"
    if args.kernel:
        # A test-only kernel (211-PRT-0002: x2APIC as firmware leaves it) in a copy of the image directory.
        copy = f"{IMAGE}-kernel"
        shutil.rmtree(ROOT / copy, ignore_errors=True)
        shutil.copytree(ROOT / IMAGE, ROOT / copy, ignore=shutil.ignore_patterns("smoke-*", "*.ppm"))
        shutil.copyfile(args.kernel, ROOT / copy / "kernel.elf")
        IMAGE = copy
    suites = ["boot", "display", "net", "tls", "netbench", "normal", "memory", "dzen", "services", "store", "storefaults", "ahci", "audio", "tts", "listen", "keys", "shell", "tools", "windows", "wm", "tablet", "usb", "vfs", "edit", "disk"] + (["busy", "smp"] if args.busy_elf else [])
    if args.isolation_elf:
        suites.append("isolation")
    if args.heap_elf:
        suites.append("heap")
    if args.block_elf:
        suites.append("block")
    if args.updater_elf:
        suites.append("updater")
    if args.arch == "aarch64":
        suites = ["normal", "shell", "vfs", "store", "storefaults", "net", "tls"] + (["busy", "smp"] if args.busy_elf else []) + (["updater"] if args.updater_elf else [])  # the suites that run on virt (issues 202-203)
    if args.suites:
        suites = args.suites.split(",")
    for suite in suites:
        if suite == "block":
            block_suite(args, args.block_elf)
            continue
        if suite == "updater":
            updater_suite(args, args.updater_elf)
            continue
        if suite == "ehci":
            ehci_suite(args)
            continue
        if suite == "hda":
            hda_suite(args)
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
        with tempfile.TemporaryDirectory(prefix="smoke-", dir=ROOT / IMAGE) as temp:
            disk = Path(temp)
            (disk / "EFI/BOOT").mkdir(parents=True)
            for name in [*(p.name for p in (ROOT / IMAGE).glob("*.elf")), BOOT_EFI]:
                shutil.copyfile(ROOT / IMAGE / name, disk / name)
            shutil.copytree(ROOT / IMAGE / "voice", disk / "voice")  # the voice recognizer's model and grammar
            if suite == "services":
                # 12 KiB for cat: three times the console's queue (000-KRN-0030).
                (disk / "lines.txt").write_text("".join(f"LINE {n:03} {'.' * 30}\n" for n in range(300)))
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
            if suite == "shell":
                (disk / "data").mkdir(exist_ok=True)
                for name, text in MSH_SCRIPTS.items():
                    (disk / "data" / name).write_text(text, encoding="utf-8")
            if suite == "wm":
                (disk / "docs").mkdir()
                (disk / "docs/notes.txt").write_text(NOTES, encoding="utf-8")
            if suite == "store":
                # blocksro: blocks asking only to read (REQUEST_BLOCKSTORE_READ 32768 for REQUEST_BLOCKSTORE 16384).
                elf = bytearray((disk / "blocks.elf").read_bytes())
                note = elf.index(b"MINDREQ1") + 8
                elf[note:note + 4] = (int.from_bytes(elf[note:note + 4], "little") & ~16384 | 32768).to_bytes(4, "little")
                (disk / "blocksro.elf").write_bytes(elf)
            if suite == "tools":
                # The dictation models' features of a test signal (250): dictate compares with kaldi-native-fbank's,
                # and runs the toy transducer on it; one byte changed in a copy fails its checksum.
                with wave.open(str(disk / "fbank.wav"), "wb") as out:
                    out.setnchannels(1); out.setsampwidth(2); out.setframerate(16000)
                    out.writeframes(struct.pack(f"<{len(FBANK_SIGNAL)}h", *FBANK_SIGNAL))
                toy = bytearray((ROOT / "tests/dictate_toy.bin").read_bytes())
                (disk / "toy.bin").write_bytes(toy)
                toy[len(toy) // 2] ^= 1
                (disk / "toy-damaged.bin").write_bytes(toy)
                # speak's front end (252): a small dictionary, its text, and a damaged copy.
                words, text, _ = speak_dictionary()
                (disk / "speak.dic").write_bytes(words)
                (disk / "speak.txt").write_text(text, encoding="utf-8")
                damaged = bytearray(words)
                damaged[len(damaged) // 2] ^= 1
                (disk / "speak-damaged.dic").write_bytes(damaged)
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
            sign_manifest.sign_volume(disk)
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
            if suite == "devicetree":
                devicetree_suite(args, disk)
                continue
            if suite == "efivar":  # aarch64 (AAVMF); on x86 the boot suite runs it
                efivar_check(args, disk)
                continue
            wav = Path(tempfile.gettempdir()) / f"mind-core-{suite}.wav" if suite in ("audio", "tts", "tablet") else "none" if suite == "listen" else None
            # The listen suite also has the launchers' network card: on QEMU's i440FX it shares the sound card's interrupt
            # line, and audio_gw must keep playing without interrupts (issue 096).
            vm = VM(args, disk.relative_to(ROOT).as_posix(),
                    rtc="2026-09-19T19:35:05" if suite == "dzen" else "localtime", audio=wav, ahci=suite == "ahci",
                    extra=["-nic", "user,model=virtio-net-pci"] if suite == "listen" else ["-gdb", f"tcp:127.0.0.1:{GDB_PORT}"] if suite == "storefaults" else (), tablet=suite == "tablet", usb_input=suite == "usb")
            try:
                if suite == "audio":
                    audio_suite(vm, wav)
                elif suite == "tts":
                    tts_suite(vm, wav, args.asr_model)
                elif suite == "listen":
                    listen_suite(vm, starts)
                elif suite == "tablet":
                    tablet_suite(vm, wav)
                else:
                    {"normal": normal_suite, "busy": busy_suite, "memory": memory_suite,
                     "smp": smp_suite, "isolation": isolation_suite, "heap": heap_suite,
                     "dzen": dzen_suite, "services": services_suite, "store": store_suite, "storefaults": store_faults_suite, "ahci": ahci_suite, "keys": keys_suite, "shell": shell_suite, "tools": tools_suite, "windows": windows_suite, "wm": wm_suite, "usb": usb_suite}[suite](vm)
            finally:
                vm.close()
                log = Path(tempfile.gettempdir()) / f"mind-core-{suite}-{args.cpus}cpu.log"
                log.write_text(vm.log)
                print(f"QEMU log: {log}", flush=True)


if __name__ == "__main__":
    main()
