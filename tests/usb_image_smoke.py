#!/usr/bin/env python3
"""Verify the packaged files, then boot the actual RAW image as a USB device (x86_64, or aarch64 with --arch aarch64)."""
import argparse
import os
from pathlib import Path
import re
import shutil
import struct
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from scripts.make_usb_image import ARCHES, LOG_SECTORS, ROOT, check_image, qemu_path, read_payloads
import qemu_smoke
from qemu_smoke import MTOOLS_ENV, VM, files_check, fsck_volume, heap_used, require, task_rows


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--arch", choices=sorted(ARCHES), default="x86_64")
    parser.add_argument("--image", type=Path, help="default: dist/mind-core-usb.img (dist/mind-core-usb-aarch64.img)")
    parser.add_argument("--qemu", default=os.environ.get("QEMU"))
    parser.add_argument("--firmware", default="OVMF.fd")
    parser.add_argument("--aavmf-code", default="/usr/share/AAVMF/AAVMF_CODE.fd")
    parser.add_argument("--aavmf-vars", default="/usr/share/AAVMF/AAVMF_VARS.fd")
    parser.add_argument("--cpus", type=int, default=4)
    args = parser.parse_args()
    args.image = args.image or ROOT / ARCHES[args.arch]["image"]
    args.qemu = args.qemu or f"qemu-system-{args.arch}"
    check_image(args.image, read_payloads(ROOT / ARCHES[args.arch]["root"], args.arch))
    qemu_smoke.IMAGE, qemu_smoke.BOOT_EFI = ARCHES[args.arch]["root"], ARCHES[args.arch]["efi"]  # what files_check compares
    with args.image.open("rb") as image:
        image.seek(450)
        assert image.read(1) == b"\xef", "USB image must mark the UEFI system partition"
    # A copy boots without a snapshot, so that what the system writes on its log partition can be read back.
    work = Path(tempfile.mkdtemp(prefix=".mind-usb-image-", dir=args.image.parent))
    booted = work / args.image.name
    shutil.copyfile(args.image, booted)
    try:
        run(args, booted)
    finally:
        shutil.rmtree(work, ignore_errors=True)


def run(args, booted):
    vm = VM(args, qemu_path(booted, args.qemu), usb=True, snapshot=False)
    try:
        # The baseline once the boot has settled (on aarch64 a service still frees memory of its start a moment later).
        baseline = heap_used(vm)
        for _ in range(10):
            time.sleep(1)
            if heap_used(vm) == baseline:
                break
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
        for _ in range(20):  # the kernel reclaims a killed task's memory as other suites wait for it
            if heap_used(vm) == baseline:
                break
            time.sleep(.1)
        assert heap_used(vm) == baseline, (heap_used(vm), baseline)
        # Files are read from the same USB drive: xHCI -> usb_host -> usb_storage -> vfs_server (issue 164).
        require(vm.service_logs("usb_host", "SUPER SPEED"), "(SUPER SPEED)")
        require(vm.service_logs("usb_storage", "[USB] MASS STORAGE: "), "[USB] MASS STORAGE: ")
        require(vm.service_logs("vfs_server", "[VFS] MOUNTED FAT16 FROM USB"), "[VFS] MOUNTED FAT16 FROM USB")
        require(vm.command("run files &"), "PID=5 NAME=files BACKGROUND")
        files_check(vm, 5)
        vm.command("kill 5")
        assert "FAULT PID=" not in vm.command("faults")
        # The log partition (211-PRT-0006, 211-KRN-0019): mounted as log:, this boot's system log saved there.
        vfs = vm.command("dmesg -s vfs_server", raw=True)  # its buffered output was read above
        require(vfs, "[VFS] MOUNTED FAT16 AT LBA ")
        name = re.search(r"GOES TO LOG:(boot\d{4}\.log)", vfs)[1]
        require(vm.command("logger LOG-PARTITION-CHECK"), "LOGGED")
        for _ in range(20):
            saved = vm.command(f"cat log:{name}", raw=True)
            if "LOG-PARTITION-CHECK" in saved:
                break
            time.sleep(.5)
        require(saved, "THE SYSTEM LOG OF ONE BOOT")
        require(saved, "LOG-PARTITION-CHECK")
        require(vm.command("write log:note.txt written on MIND CORE"), "WROTE")
        require(vm.command("sync"), "OK")
        time.sleep(3)  # the journal's last save, flushed
    finally:
        vm.close()
        log = Path(tempfile.gettempdir()) / f"mind-core-usb-image{'' if args.arch == 'x86_64' else '-' + args.arch}.log"
        log.write_text(vm.log)
        print(f"QEMU log: {log}")
    # On the host, as any computer reads it: the boot log and the note on a clean FAT16 volume.
    with booted.open("rb") as image:
        mbr = image.read(512)
    assert mbr[466] == 0x0E, "the log partition"
    start = struct.unpack_from("<I", mbr, 470)[0]
    read = lambda file: subprocess.run(["mtype", "-i", f"{booted}@@{start * 512}", f"::{file}"], check=True, capture_output=True, env=MTOOLS_ENV).stdout
    require(read(name).decode(errors="replace"), "LOG-PARTITION-CHECK")
    assert read("note.txt") == b"written on MIND CORE\n", read("note.txt")
    # The hardware report (174-KRN-0038): this boot's beside its system log, and the firmware's ACPI tables.
    report = read(f"hw{name[4:8]}.txt").decode(errors="replace")
    for section in ("MIND CORE HARDWARE REPORT 1", "\nCPU\n", "Memory map (the firmware's)", "ACPI tables", "PCI functions", "The kernel's choices", "Kernel lines", "MIND CORE KERNEL: INIT STARTED"):
        assert section in report, (section, report[:2000])
    assert re.search(r"USB controller", report), report
    for table in ("FACP", "DSDT", "APIC"):
        data = read(f"acpi/{table}.bin")
        assert data[:4] == table.encode() and len(data) == struct.unpack_from("<I", data, 4)[0], (table, len(data))
    assert read("acpi/RSDP.bin").startswith(b"RSD PTR ")
    fsck_volume(booted, start, LOG_SECTORS)
    print(f"PASS ({args.arch}): exact image contents; UEFI boot from USB RAW image; CPUs, all programs, private heap, fg/exit/kill/reclaim; "
          "VFS over xHCI USB mass storage through usb_host; the boot's system log, the hardware report, the ACPI tables and a file on the log partition, read on the host")


if __name__ == "__main__":
    main()
