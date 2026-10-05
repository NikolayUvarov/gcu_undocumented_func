#!/usr/bin/env python3
"""aarch64 on QEMU `virt` (issue 201): boots to `[INIT] READY` on the PL011 console with the services that need no
devices, and a service that faults is ended, restarted and quarantined while the rest keeps running.

Needs `scripts/build_aarch64.sh --fixtures` first, qemu-system-aarch64 and AAVMF (qemu-efi-aarch64)."""
import argparse
import re
import shutil
import subprocess
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BUILD = ROOT / "aarch64_root"
# What each fault-test variant does and the exception class (ESR_EL1.EC) the kernel reports for it.
CASES = {"kernel_read": 0x24, "text_write": 0x24, "stack_exec": 0x20, "undefined": 0x00}


def boot(args, disk, until, timeout=90):
    """Boots QEMU on `disk` and returns the console output once `until` appears (or the timeout passes)."""
    variables = Path(tempfile.mkdtemp()) / "vars.fd"
    shutil.copyfile(args.vars, variables)
    process = subprocess.Popen(
        [args.qemu, "-machine", "virt,gic-version=3", "-cpu", "max", "-m", "512", "-nographic", "-no-reboot",
         "-drive", f"if=pflash,format=raw,readonly=on,file={args.code}", "-drive", f"if=pflash,format=raw,file={variables}",
         "-drive", f"format=raw,file=fat:rw:{disk}", "-device", "ramfb", "-nic", "none"],
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
    return output.decode(errors="replace")


def disk_with(rtc=None):
    disk = Path(tempfile.mkdtemp()) / "root"
    shutil.copytree(BUILD, disk, ignore=shutil.ignore_patterns("fault-*.elf"))
    if rtc:
        shutil.copyfile(BUILD / f"fault-{rtc}.elf", disk / "rtc.elf")
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
    for line in ("MIND CORE KERNEL: INIT STARTED", "[INIT] STARTED logd", "[LOGD] READY", "[INIT] STARTED loader", "[LOADER] READY",
                 "[INIT] STARTED keystore", "[INIT] STARTED sysmon", "[INIT] READY", "[SYSMON] READY"):
        require(output, line)
    assert "KERNEL PANIC" not in output and "KERNEL EXCEPTION" not in output, output
    entropy = "[KEYSTORE] DEVICE KEY READY" in output
    print(f"PASS: aarch64 boot to [INIT] READY on the PL011 console; logd, loader, keystore ({'device key from RNDR' if entropy else 'no RNDR'}) and sysmon run", flush=True)

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
