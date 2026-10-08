#!/usr/bin/env python3
"""Secure Boot with our own keys in QEMU (351-UPD-0012; MC-9.1, 9.4): OVMF's Secure Boot build, with a test PK, KEK
and db made for the run and enrolled in a fresh variable store, runs the bootloader signed with that db key, and
refuses the unsigned one, one signed with another key, and the signed one once its hash is in dbx.

Usage: secure_boot_smoke.py [--code OVMF_CODE_4M.secboot.fd] [--vars OVMF_VARS_4M.fd] [--qemu qemu-system-x86_64]
Needs what scripts/secure_boot.py needs; a missing tool fails the run, it is not skipped.
"""
import argparse
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
import secure_boot  # noqa: E402
import sign_manifest  # noqa: E402

LOADER = "EFI/BOOT/BOOTX64.EFI"


def boot(args, volume, variables, until, timeout=90):
    """Boots the volume with the variable store; the serial output up to `until` (or the timeout)."""
    command = [args.qemu, "-machine", "q35,smm=on", "-global", "driver=cfi.pflash01,property=secure,value=on",
               "-drive", f"if=pflash,format=raw,unit=0,readonly=on,file={args.code}",
               "-drive", f"if=pflash,format=raw,unit=1,file={variables}",
               "-drive", f"format=raw,file=fat:{volume},snapshot=on", "-m", "512", "-smp", "2", "-nic", "none",
               "-serial", "stdio", "-display", "none", "-no-reboot"]
    process = subprocess.Popen(command, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    output, deadline = b"", time.monotonic() + timeout
    import os
    os.set_blocking(process.stdout.fileno(), False)
    try:
        while time.monotonic() < deadline and not any(u in output for u in until) and process.poll() is None:
            chunk = process.stdout.read(4096)
            output += chunk or b""
            if not chunk:
                time.sleep(.1)
    finally:
        process.kill()
        process.wait()
    return output.decode(errors="replace")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--qemu", default="qemu-system-x86_64")
    parser.add_argument("--code", default="/usr/share/OVMF/OVMF_CODE_4M.secboot.fd")
    parser.add_argument("--vars", default="/usr/share/OVMF/OVMF_VARS_4M.fd")
    args = parser.parse_args()
    for tool in ("openssl", "sbsign", args.qemu):
        assert shutil.which(tool), f"{tool} is not installed"
    for f in (args.code, args.vars):
        assert Path(f).exists(), f"{f} is missing"
    with tempfile.TemporaryDirectory(prefix="mind-sb-") as temp:
        temp = Path(temp)
        keys, other = secure_boot.keys(temp / "keys"), secure_boot.keys(temp / "other", "Not MIND Core")
        unsigned = ROOT / "usb_root" / LOADER
        volume = temp / "volume"
        shutil.copytree(ROOT / "usb_root", volume, ignore=shutil.ignore_patterns("MANIFEST*"))

        def with_loader(loader):
            # The loader the firmware will run, and a manifest signed over the volume as it then is.
            shutil.copyfile(loader, volume / LOADER)
            sign_manifest.sign_volume(volume)
        signed = secure_boot.sign(unsigned, keys, temp / "signed.efi")
        foreign = secure_boot.sign(unsigned, other, temp / "foreign.efi")
        enrolled = secure_boot.variables(args.vars, keys, temp / "vars.fd")
        revoked = secure_boot.variables(args.vars, keys, temp / "vars-revoked.fd", revoke=[signed])
        started = ("MIND CORE KERNEL: INIT STARTED".encode(),)
        refused = (b"Access Denied", b"BOOT:")

        with_loader(signed)
        out = boot(args, volume, shutil.copyfile(enrolled, temp / "run.fd"), started + (b"BOOT ERROR",))
        assert "BOOT: MANIFEST" in out and "MIND CORE KERNEL: INIT STARTED" in out, out[-3000:]
        print("PASS: Secure Boot with our keys: the bootloader signed with our db key boots the system", flush=True)
        for name, loader, store in (("unsigned", unsigned, enrolled), ("signed with another key", foreign, enrolled), ("revoked in dbx", signed, revoked)):
            with_loader(loader)
            out = boot(args, volume, shutil.copyfile(store, temp / "run.fd"), refused, timeout=60)
            assert "BOOT:" not in out and "MIND CORE KERNEL" not in out, (name, out[-3000:])
            # OVMF names the refusal on the serial console: the image failed verification against db and dbx.
            assert "Access Denied" in out, (name, out[-3000:])
            print(f"PASS: Secure Boot with our keys: the firmware refuses the bootloader {name}", flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
