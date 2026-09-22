#!/usr/bin/env python3
"""Verify the packaged files, then boot the actual RAW image as a USB device."""
import argparse
import os
from pathlib import Path
import sys
import tempfile
import time

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from scripts.make_usb_image import ROOT, check_image, qemu_path, read_payloads
from qemu_smoke import VM, heap_used, require, task_rows


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--image", type=Path, default=ROOT / "dist/mind-core-usb.img")
    parser.add_argument("--qemu", default=os.environ.get("QEMU", "qemu-system-x86_64"))
    parser.add_argument("--firmware", default="OVMF.fd")
    parser.add_argument("--cpus", type=int, default=4)
    args = parser.parse_args()
    check_image(args.image, read_payloads(ROOT / "usb_root"))
    with args.image.open("rb") as image:
        image.seek(450)
        assert image.read(1) == b"\xef", "USB image must mark the UEFI system partition"
    vm = VM(args, qemu_path(args.image, args.qemu), usb=True)
    try:
        baseline = heap_used(vm)
        cpus = vm.command("cpus")
        assert cpus.count("ONLINE=true") == args.cpus, cpus
        listing = vm.command("list")
        for name in ["app", "app2", "clock", "dzen-clock"]:
            require(listing, name)
        for pid, name in enumerate(["app", "app2", "clock", "dzen-clock"], 1):
            require(vm.command(f"run {name} &"), f"PID={pid} NAME={name} BACKGROUND")
        first = task_rows(vm)
        time.sleep(.4)
        second = task_rows(vm)
        assert set(second) == {1, 2, 3, 4}, second
        assert all(int(second[p][-1]) > int(first[p][-1]) for p in second), (first, second)
        require(vm.command("logs 2"), "PRIVATE HEAP SPRITE READY")
        require(vm.command("logs 3"), "[CLOCK]")
        require(vm.command("logs 4"), "[DZEN-CLOCK]")
        vm.send("fg 2\n")
        vm.expect("FOREGROUND PID=2")
        vm.send("\x1b\n")
        vm.expect("PID=2 EXITED. SHELL RESUMED.")
        time.sleep(.1); vm.collect(); vm.output = ""
        for pid in [1, 3, 4]:
            require(vm.command(f"kill {pid}"), f"KILLED PID={pid}")
        assert heap_used(vm) == baseline
        assert "FAULT PID=" not in vm.command("faults")
        print("PASS: exact image contents; UEFI boot from USB RAW image; CPUs, all programs, private heap, fg/exit/kill/reclaim")
    finally:
        vm.close()
        log = Path(tempfile.gettempdir()) / "mind-core-usb-image.log"
        log.write_text(vm.log)
        print(f"QEMU log: {log}")


if __name__ == "__main__":
    main()
