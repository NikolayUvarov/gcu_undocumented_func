#!/usr/bin/env python3
"""A dbx update from the running system (351-KRN-0028; MC-9.4). OVMF's Secure Boot build with a test PK, KEK and db
made for the run: the bootloader signed with our db key boots; in the system, `efivar append dbx` hands the firmware a
signature list with that bootloader's hash. Unsigned, the firmware refuses it; signed with our KEK, it takes it, and the
next boot refuses the bootloader.

Usage: dbx_update_smoke.py [--code OVMF_CODE_4M.secboot.fd] [--vars OVMF_VARS_4M.fd] [--qemu qemu-system-x86_64]
Needs what scripts/secure_boot.py needs, and sbvarsign (sbsigntool); a missing tool fails the run.
"""
import argparse
import os
import shutil
import struct
import subprocess
import sys
import tempfile
import time
import uuid
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
import secure_boot  # noqa: E402
import sign_manifest  # noqa: E402

LOADER = "EFI/BOOT/BOOTX64.EFI"
CERT_SHA256 = uuid.UUID("c1c41626-504c-4092-aca9-41f936934328")


def signature_list(digest, owner):
    """An EFI_SIGNATURE_LIST of one SHA-256 hash (hex) owned by `owner`."""
    data = uuid.UUID(owner).bytes_le + bytes.fromhex(digest)
    return CERT_SHA256.bytes_le + struct.pack("<III", 28 + len(data), 0, len(data)) + data


class Machine:
    """The Secure Boot machine on the serial line, which the shell reads too; it may reset (no -no-reboot)."""
    def __init__(self, args, volume, variables):
        command = [args.qemu, "-machine", "q35,smm=on", "-global", "driver=cfi.pflash01,property=secure,value=on",
                   "-drive", f"if=pflash,format=raw,unit=0,readonly=on,file={args.code}",
                   "-drive", f"if=pflash,format=raw,unit=1,file={variables}",
                   "-drive", f"format=raw,file=fat:{volume},snapshot=on", "-m", "512", "-smp", "2", "-nic", "none",
                   "-serial", "stdio", "-display", "none"]
        self.process = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
        os.set_blocking(self.process.stdout.fileno(), False)
        self.output, self.log = "", ""

    def expect(self, text, timeout=60):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            chunk = self.process.stdout.read(4096)
            if chunk:
                self.output += chunk.decode(errors="replace").replace("\r", "")
                self.log += chunk.decode(errors="replace")
            if text in self.output:
                seen, self.output = self.output, ""
                return seen
            if not chunk:
                time.sleep(.05)
        raise AssertionError(f"timeout waiting for {text!r}: {self.log[-3000:]}")

    def send(self, text):
        for byte in text.encode():
            self.process.stdin.write(bytes([byte]))
            self.process.stdin.flush()
            time.sleep(.01)

    def efivar(self, line):
        self.send(line + "\n")
        self.expect("ALLOW? (Y/N)", timeout=30)
        self.send("y")
        return self.expect("MIND> ", timeout=60)

    def close(self):
        self.process.kill()
        self.process.wait()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--qemu", default="qemu-system-x86_64")
    parser.add_argument("--code", default="/usr/share/OVMF/OVMF_CODE_4M.secboot.fd")
    parser.add_argument("--vars", default="/usr/share/OVMF/OVMF_VARS_4M.fd")
    args = parser.parse_args()
    for tool in ("openssl", "sbsign", "sbvarsign", args.qemu):
        assert shutil.which(tool), f"{tool} is not installed"
    for f in (args.code, args.vars):
        assert Path(f).exists(), f"{f} is missing"
    with tempfile.TemporaryDirectory(prefix="mind-dbx-") as temp:
        temp = Path(temp)
        keys = secure_boot.keys(temp / "keys")
        volume = temp / "volume"
        shutil.copytree(ROOT / "usb_root", volume, ignore=shutil.ignore_patterns("MANIFEST*"))
        signed = secure_boot.sign(ROOT / "usb_root" / LOADER, keys, temp / "signed.efi")
        shutil.copyfile(signed, volume / LOADER)
        # The update: the signed bootloader's hash, as a bare list and as sbvarsign signs it with our KEK.
        (volume / "dbx.esl").write_bytes(signature_list(secure_boot.authenticode(signed), secure_boot.OWNER))
        subprocess.run(["sbvarsign", "--key", str(keys / "KEK.key"), "--cert", str(keys / "KEK.crt"),
                        "--attr", "NON_VOLATILE,BOOTSERVICE_ACCESS,RUNTIME_ACCESS,TIME_BASED_AUTHENTICATED_WRITE_ACCESS,APPEND_WRITE",
                        "--output", str(volume / "dbx.auth"), "dbx", str(volume / "dbx.esl")], check=True, capture_output=True)
        sign_manifest.sign_volume(volume)
        machine = Machine(args, volume, secure_boot.variables(args.vars, keys, temp / "vars.fd"))
        try:
            machine.expect("MIND> ", timeout=120)
            refused = machine.efivar("efivar append dbx dbx.esl")
            assert "efivar: dbx: " in refused and "APPENDED" not in refused, refused
            taken = machine.efivar("efivar append dbx dbx.auth")
            assert "dbx APPENDED: " in taken, taken
            machine.send("reboot\n")
            machine.expect("REBOOTING", timeout=60)
            after = machine.expect("Access Denied", timeout=120)
            assert "MIND CORE KERNEL: INIT STARTED" not in after and "BOOT: MANIFEST" not in after, after[-3000:]
        finally:
            machine.close()
            (Path(tempfile.gettempdir()) / "mind-core-dbx-update.log").write_text(machine.log)
    print("PASS: a dbx update from the system: an unsigned signature list refused by the firmware; the one signed with "
          "our KEK taken, and the next boot refuses the bootloader it revokes", flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
