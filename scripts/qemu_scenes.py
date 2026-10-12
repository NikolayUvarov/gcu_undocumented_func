#!/usr/bin/env python3
"""The showcase page's pictures from QEMU (000-KRN-0093): the current x86 build (usb_root) boots in the test suites'
machine (tests/qemu_smoke.py), plays fixed scenes and saves each screen as PNG, so the page's pictures are made the same
way for each of its updates. They show QEMU's machine, not a real one (MC-12.1): the page says so under each.

  qemu_scenes.py [-o DIR] [--firmware OVMF.fd] [--size 1920x1080] [--scenes boot,check,top,wm,series,tour]

Scenes: `boot` the shell after the boot; `check` the kernel's self-test table; `top`; `wm` with fm, clock, top and the
shell's window; `series` wm's windows changing, a frame every half second, also as an animated WebP and GIF;
`tour` the page's animation: the boot, `check`, `hw`, `top`, then `wm` and three of its windows full screen, each going
into the next through dark (tour.webp, lossless, 1280 wide: the shell's screens cut at their top left, wm's scaled;
scripts/media.py, ffmpeg). Run `tour` alone: it plays its own scenes."""
import argparse
import re
import shutil
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tests"))
sys.path.insert(0, str(ROOT / "scripts"))
import media  # noqa: E402
import qemu_smoke as smoke  # noqa: E402
import sign_manifest  # noqa: E402

SCENES = ("boot", "check", "top", "wm", "series", "tour")
PAGE = (1280, 720)  # the tour's pictures


def ppm(data):
    """QEMU's screen dump (binary PPM, 8 bits a channel): (width, height, RGB bytes)."""
    fields, at = [], 0
    while len(fields) < 4:
        while data[at:at + 1].isspace():
            at += 1
        if data[at:at + 1] == b"#":
            at = data.index(b"\n", at)
            continue
        end = at
        while not data[end:end + 1].isspace():
            end += 1
        fields.append(data[at:end])
        at = end
    if fields[0] != b"P6" or fields[3] != b"255":
        raise ValueError(f"a screen dump of {fields[0]!r}, {fields[3]!r} levels")
    width, height = int(fields[1]), int(fields[2])
    return width, height, data[at + 1:at + 1 + width * height * 3]


class Scenes:
    def __init__(self, vm, out):
        self.vm, self.out = vm, out

    def shot(self, name):
        width, height, rgb = ppm(self.vm.screenshot())
        self.vm.serial(enter=False)  # the program in front gets no Enter
        path = self.out / f"{name}.png"
        path.write_bytes(media.png(width, height, rgb))
        media.report(path, f", {width}x{height}")
        return path

    def boot(self):
        time.sleep(1)
        self.shot("boot")

    def check(self):
        self.vm.command("clear")
        self.vm.send("check kernel\n")
        self.vm.expect("MIND> ", timeout=300, after="check kernel\n")
        self.shot("check")

    def top(self):
        self.vm.command("run clock &")
        self.vm.send("top\n")
        self.vm.expect("[TOP] READY")
        time.sleep(2)
        self.shot("top")
        self.vm.send("q")
        self.vm.expect("MIND> ")

    def start_wm(self):
        if getattr(self, "wm_up", False):
            return
        self.vm.send("wm fm /, clock, top\n")
        self.vm.expect("[WM] MODE=", timeout=30)
        deadline = time.monotonic() + 30
        while len(re.findall(r"\[WM\] WINDOW \d+ PID", self.vm.log)) < 3 and time.monotonic() < deadline:
            time.sleep(.2)
            self.vm.collect()
        self.vm.hmp("sendkey ctrl-alt-f5")  # the shell's own window
        self.vm.serial(enter=False)
        time.sleep(2)
        self.wm_up = True

    def wm(self):
        self.start_wm()
        self.shot("wm")

    def keys(self, *names):
        for name in names:
            self.vm.hmp(f"sendkey {name}")
            time.sleep(.08)
        self.vm.serial(enter=False)

    def program(self, picture, name, ready, quit):
        # `name` in front of the shell, its screen as `picture` once `ready` was printed and 2 s passed; then `quit` keys.
        self.vm.command("clear")
        self.vm.send(name + "\n")
        self.vm.expect(ready, timeout=30)
        time.sleep(2)
        path = self.shot(picture)
        self.vm.send_bytes(quit)
        self.vm.expect("MIND> ", timeout=30)
        return path

    def tour(self):
        (self.out / "tour").mkdir(exist_ok=True)
        shots = [(self.shot("tour/1-boot"), "crop")]
        self.vm.command("clear")
        self.vm.send("check kernel\n")
        self.vm.expect("MIND> ", timeout=300, after="check kernel\n")
        shots.append((self.shot("tour/2-check"), "crop"))
        shots.append((self.program("tour/3-hw", "hw", "[HW] READY", b"q"), "crop"))
        self.vm.command("run clock &")
        shots.append((self.program("tour/4-top", "top", "[TOP] READY", b"q"), "crop"))
        self.vm.command("clear")
        self.start_wm()
        shots.append((self.shot("tour/5-wm"), "scale"))
        # Alt+Tab to each of fm, clock and top, Alt+F: the window fills the screen; Alt+F again gives it back.
        for n in range(3):
            self.keys("alt-tab")
            time.sleep(.6)
            self.keys("alt-f")
            time.sleep(1.5)
            shots.append((self.shot(f"tour/{6 + n}-full"), "scale"))
            self.keys("alt-f")
            time.sleep(.6)
        pages = []
        for n, (path, fit) in enumerate(shots):
            page = self.out / "tour" / f"page-{n + 1:02d}.png"
            view = f"crop={PAGE[0]}:{PAGE[1]}:0:0" if fit == "crop" else f"scale={PAGE[0]}:{PAGE[1]}:flags=lanczos"
            media.ffmpeg("-i", path, "-vf", view, page, why="the tour's pictures")
            pages.append(page)
        media.slideshow(pages, self.out / "tour", PAGE[0], hold=3.0, fade=0.6)

    def series(self):
        # Alt+Tab brings each window to the front, Alt+F fills the screen with one and gives the screen back.
        self.start_wm()
        frames = self.out / "series"
        shutil.rmtree(frames, ignore_errors=True)
        frames.mkdir()
        steps = [(), ("alt-tab",), (), ("alt-tab",), (), ("alt-f",), (), ("alt-f",), (), ("alt-tab",), (), ()]
        for n, keys in enumerate(steps):
            started = time.monotonic()
            self.keys(*keys)
            width, height, rgb = ppm(self.vm.screenshot())
            self.vm.serial(enter=False)
            (frames / f"frame-{n + 1:04d}.png").write_bytes(media.png(width, height, rgb))
            time.sleep(max(0, .5 - (time.monotonic() - started)))
        print(f"{frames}/: {len(steps)} PNG files", flush=True)
        media.animate(["-framerate", "2", "-i", frames / "frame-%04d.png"], "2", 960, frames)


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("-o", default=str(ROOT / "media-scenes"), help="output directory")
    parser.add_argument("--qemu", default="qemu-system-x86_64")
    parser.add_argument("--firmware", default="/usr/share/ovmf/OVMF.fd")
    parser.add_argument("--cpus", type=int, default=4)
    parser.add_argument("--size", default="1920x1080", help="the screen QEMU's VGA offers the firmware")
    parser.add_argument("--scenes", default=",".join(SCENES))
    args = parser.parse_args()
    wanted = args.scenes.split(",")
    if unknown := [s for s in wanted if s not in SCENES]:
        parser.error(f"no scene {', '.join(unknown)}")
    width, height = map(int, args.size.split("x"))
    out = Path(args.o)
    out.mkdir(parents=True, exist_ok=True)
    started = time.monotonic()
    with tempfile.TemporaryDirectory(prefix="smoke-scenes-", dir=ROOT / smoke.IMAGE) as temp:
        disk = Path(temp)
        (disk / "EFI/BOOT").mkdir(parents=True)
        for name in [*(p.name for p in (ROOT / smoke.IMAGE).glob("*.elf")), smoke.BOOT_EFI]:
            shutil.copyfile(ROOT / smoke.IMAGE / name, disk / name)
        shutil.copytree(ROOT / smoke.IMAGE / "voice", disk / "voice")
        sign_manifest.sign_volume(disk)
        vm = smoke.VM(args, disk.relative_to(ROOT).as_posix(), prompt=False, extra=["-vga", "none", "-device", f"VGA,xres={width},yres={height},edid=on"])
        try:
            vm.expect("MIND> ", timeout=90)  # the boot screen as it is, without the suites' `ps`
            scenes = Scenes(vm, out)
            for name in wanted:
                getattr(scenes, name)()
        finally:
            vm.close()
            log = out / "qemu.log"
            log.write_text(vm.log)
    print(f"{len(wanted)} scenes in {time.monotonic() - started:.0f} s; QEMU's output in {log}", flush=True)


if __name__ == "__main__":
    main()
