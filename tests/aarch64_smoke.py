#!/usr/bin/env python3
"""aarch64 on QEMU `virt` (issue 201): boots to `[INIT] READY` on the PL011 console with the services that need no
devices, and a service that faults is ended, restarted and quarantined while the rest keeps running. The disk has no
shell, so the kernel keeps mirroring every log to the PL011 (the shell would take it over), and no PCI (`highmem` on
puts the ECAM above 4 GiB): the boot path without devices.

Needs `scripts/build_aarch64.sh --fixtures` first, qemu-system-aarch64 and AAVMF (qemu-efi-aarch64)."""
import argparse
import atexit
import re
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BUILD = ROOT / "aarch64_root"
sys.path.insert(0, str(ROOT / "scripts"))
import sign_manifest  # noqa: E402
import boot_slots_check  # noqa: E402
# What each fault-test variant does and the exception class (ESR_EL1.EC) the kernel reports for it.
CASES = {"kernel_read": 0x24, "text_write": 0x24, "stack_exec": 0x20, "undefined": 0x00}


def boot(args, disk, until, timeout=90, decoy=None, raw=False):
    """Boots QEMU on `disk` and returns the console output once `until` appears (or the timeout passes). AAVMF under
    TCG now and then stalls before it loads the bootloader; a boot that shows no kernel line is tried once more.
    `decoy`: a directory served as another FAT disk ahead of `disk` (211-KRN-0012). `raw`: `disk` is a disk image."""
    output = boot_once(args, disk, until, timeout, decoy, raw)
    if "MIND CORE KERNEL" not in output and "BdsDxe: starting" not in output:
        print("NOTE: the firmware stalled before loading the bootloader; booting again", flush=True)
        output = boot_once(args, disk, until, timeout, decoy, raw)
    return output


def boot_once(args, disk, until, timeout, decoy=None, raw=False):
    variables = Path(tempfile.mkdtemp()) / "vars.fd"
    shutil.copyfile(args.vars, variables)
    process = subprocess.Popen(
        [args.qemu, "-machine", "virt,gic-version=3", "-cpu", "max", "-m", "512", "-nographic", "-no-reboot",
         "-drive", f"if=pflash,format=raw,readonly=on,file={args.code}", "-drive", f"if=pflash,format=raw,file={variables}",
         *(["-drive", f"format=raw,file=fat:{decoy},readonly=on"] if decoy else []),
         "-drive", f"format=raw,file={disk}" if raw else f"format=raw,file=fat:rw:{disk}", "-device", "ramfb", "-nic", "none"],
        stdout=subprocess.PIPE, stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL)
    output, deadline = b"", time.monotonic() + timeout
    try:
        import os, select
        os.set_blocking(process.stdout.fileno(), False)
        settle = None  # read on for a second after `until`, so lines printed around it are in
        while time.monotonic() < (settle or deadline):
            ready, _, _ = select.select([process.stdout], [], [], 0.2)
            if ready:
                output += process.stdout.read() or b""
            if settle is None and until.encode() in output:
                settle = time.monotonic() + 1
    finally:
        process.kill()
        process.wait()
        shutil.rmtree(variables.parent, ignore_errors=True)  # 64 MiB a boot
    return output.decode(errors="replace")


def disk_with(rtc=None):
    disk = Path(tempfile.mkdtemp()) / "root"
    atexit.register(shutil.rmtree, disk.parent, True)  # a copy of the build: removed when the test ends
    shutil.copytree(BUILD, disk, ignore=shutil.ignore_patterns("fault-*.elf", "shell.elf"))
    if rtc:
        shutil.copyfile(BUILD / f"fault-{rtc}.elf", disk / "rtc.elf")
    sign_manifest.sign_volume(disk)  # signed as the build signs it (350-UPD-0002)
    return disk


def require(text, fragment):
    assert fragment in text, (fragment, text[-3000:])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--qemu", default="qemu-system-aarch64")
    parser.add_argument("--code", default="/usr/share/AAVMF/AAVMF_CODE.fd")
    parser.add_argument("--vars", default="/usr/share/AAVMF/AAVMF_VARS.fd")
    args = parser.parse_args()
    log = Path(tempfile.gettempdir()) / "mind-core-aarch64.log"

    output = boot(args, disk_with(), "[INIT] READY")
    log.write_text(output)
    for line in ("MIND CORE BOOT: USING GOP ", "MIND CORE BOOT: 1 CPUS; EXITING BOOT SERVICES",  # 211-KRN-0016
                 "MIND CORE KERNEL: INIT STARTED", "[INIT] STARTED logd", "[LOGD] READY", "[INIT] STARTED loader", "[LOADER] READY",
                 "[INIT] STARTED keystore", "[INIT] STARTED sysmon", "[INIT] READY", "[SYSMON] READY",
                 # 300-KRN-0001: the block store over its own RAM disk.
                 "[INIT] STARTED ramdisk#1", "[BLOCKSTORE] READY BLOCKS=0 NAMES=0 SECTORS=1/16384 CORRUPT=0 DAMAGED=0"):
        require(output, line)
    assert "KERNEL PANIC" not in output and "KERNEL EXCEPTION" not in output, output
    # A line goes out in one write (issue 209): no service's line lands inside init's.
    assert not re.search(r"^\[INIT\] STARTED \[", output, re.M), output
    # 350-UPD-0004: the launch record in the system log is the bootloader's serial line.
    serial = re.search(r"BOOT: MANIFEST (\S+ KEY \S+(?: \(THE TEST KEY\))? VERIFIED, \d+ IMAGES CHECKED)", output)
    assert serial, output[-3000:]
    require(output, f"[INIT] LAUNCH: MANIFEST {serial[1]}; THE VOLUME'S ROOT")
    entropy = "[KEYSTORE] DEVICE KEY READY" in output
    print(f"PASS: aarch64 boot to [INIT] READY on the PL011 console; logd, loader, keystore ({'device key from RNDR' if entropy else 'no RNDR'}) and sysmon run", flush=True)

    # 211-KRN-0012: the firmware lists another disk's EFI partition first; the loader reads its own volume.
    decoy = Path(tempfile.mkdtemp()) / "decoy"
    atexit.register(shutil.rmtree, decoy.parent, True)
    (decoy / "EFI/APPLE").mkdir(parents=True)
    output = boot(args, disk_with(), "[INIT] READY", decoy=decoy)
    with log.open("a") as file:
        file.write(f"\n=== decoy disk first\n{output}")
    require(output, "[INIT] READY")
    assert "BOOT ERROR" not in output, output[-3000:]
    # Each VirtIO disk has its virtio_blk instance (211-DRV-0009): vfs_server gets both and mounts the boot volume,
    # the second disk, holding the manifest the bootloader verified (211-KRN-0012).
    require(output, "BOOT: VOLUME MBR PARTITION 1 AT LBA 63")
    require(output, "[VFS] MOUNTED FAT16 FROM VIRTIO AT LBA 63")
    require(output, "[VFS] THE BOOT VOLUME: MBR DISK BE1AFDFA, PARTITION 1 AT LBA 63, AND THE MANIFEST THE BOOTLOADER VERIFIED")
    print("PASS: aarch64 bootloader reads its own volume when the firmware lists another disk's FAT volume first; "
          "vfs_server mounts that volume from the second VirtIO disk", flush=True)

    # 351-UPD-0006: slots A and B on a raw disk, where the bootloader counts a trial's tries and falls back.
    slots = Path(tempfile.mkdtemp(prefix="mind-slots-"))
    atexit.register(shutil.rmtree, slots, True)
    boot_slots_check.run(lambda image, until: boot(args, image, until, raw=True), slots, disk_with(), "aarch64")

    for case, code in CASES.items():
        output = boot(args, disk_with(case), "QUARANTINED", timeout=120)
        with log.open("a") as file:
            file.write(f"\n=== {case}\n{output}")
        require(output, "[FAULTTEST] RUNNING")
        require(output, "[INIT] READY")
        faults = [int(v) for v in re.findall(r"\[INIT\] rtc PID=\d+ FAULT VECTOR (\d+)", output)]
        assert len(faults) >= 3 and set(faults) == {code}, (case, faults, output[-2000:])
        require(output, "[INIT] rtc QUARANTINED")
        assert "NOT STOPPED" not in output and "KERNEL PANIC" not in output and "KERNEL EXCEPTION" not in output, output
        require(output, "[SYSMON] READY")
    print("PASS: aarch64 fault containment: kernel read, code write, stack execution and an undefined instruction end the task "
          "with its exception class; init restarts it and quarantines it after 3 restarts; the other services keep running", flush=True)
    print(f"QEMU log: {log}", flush=True)


if __name__ == "__main__":
    main()
