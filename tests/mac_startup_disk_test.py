"""scripts/mac/startup-disk.sh against stand-ins of macOS's bless and diskutil, under sh and bash (800-LNK-0029)."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

SCRIPT = Path(__file__).resolve().parents[1] / "scripts" / "mac" / "startup-disk.sh"
NODE = "/dev/disk2s1"
# bless logs each call; --getBoot answers $GETBOOT, the rest fail with $BLESS_STATUS.
BLESS = """#!/bin/sh
echo "$*" >> "$CALLS"
if [ "$1" = --getBoot ]; then echo "$GETBOOT"; exit 0; fi
[ "${BLESS_STATUS:-0}" = 0 ] || echo "Could not set boot device property: 0xe00002e2" >&2
exit "${BLESS_STATUS:-0}"
"""
# diskutil info as macOS prints it, for the volume given in $VOLUME alone.
DISKUTIL = """#!/bin/sh
[ "$1 $2" = "info $VOLUME" ] || { echo "Could not find disk: $2" >&2; exit 1; }
printf '   Device Identifier:         disk2s1\\n   Device Node:               %s\\n   Whole:                     No\\n' "$NODE"
"""
SHELLS = [s for s in ("sh", "bash") if shutil.which(s)]


class StartupDisk(unittest.TestCase):
    def setUp(self):
        self.dir = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, self.dir)
        self.volume = (self.dir / "MINDCORE").resolve()
        (self.volume / "EFI" / "BOOT").mkdir(parents=True)
        (self.volume / "EFI" / "BOOT" / "BOOTX64.EFI").write_bytes(b"MZ")
        (self.volume / "mac").mkdir()
        shutil.copy(SCRIPT, self.volume / "mac" / "startup-disk.sh")
        bin_ = self.dir / "bin"
        bin_.mkdir()
        for name, text in (("bless", BLESS), ("diskutil", DISKUTIL)):
            (bin_ / name).write_text(text)
            (bin_ / name).chmod(0o755)
        self.calls = self.dir / "calls"
        self.env = {**os.environ, "PATH": f"{bin_}:{os.environ['PATH']}", "CALLS": str(self.calls), "VOLUME": str(self.volume),
                    "NODE": NODE, "GETBOOT": NODE}

    def run_script(self, shell, cwd=None, script=None, **env):
        script = script or str(self.volume / "mac" / "startup-disk.sh")
        return subprocess.run([shell, script], cwd=cwd or self.dir, env={**self.env, **env}, capture_output=True, text=True)

    def bless_calls(self):
        return self.calls.read_text().splitlines() if self.calls.exists() else []

    def test_blesses_the_loader_of_its_own_volume(self):
        for shell in SHELLS:
            with self.subTest(shell=shell):
                self.calls.unlink(missing_ok=True)
                done = self.run_script(shell)
                self.assertEqual(done.returncode, 0, done.stderr)
                loader = self.volume / "EFI" / "BOOT" / "BOOTX64.EFI"
                self.assertEqual(self.bless_calls(), [f"--mount {self.volume} --setBoot --file {loader} --shortform", "--getBoot"])
                self.assertIn(f"the Mac starts {NODE}", done.stdout)

    def test_run_from_the_volume_by_a_relative_path(self):
        for shell in SHELLS:
            with self.subTest(shell=shell):
                self.calls.unlink(missing_ok=True)
                done = self.run_script(shell, cwd=self.volume, script="mac/startup-disk.sh")
                self.assertEqual(done.returncode, 0, done.stderr)
                self.assertEqual(self.bless_calls()[0].split()[1], str(self.volume))

    def test_refused_outside_recovery(self):
        for shell in SHELLS:
            with self.subTest(shell=shell):
                done = self.run_script(shell, BLESS_STATUS="3")
                self.assertNotEqual(done.returncode, 0)
                self.assertIn("macOS Recovery", done.stderr)
                self.assertNotIn("the Mac starts", done.stdout)

    def test_another_boot_device_is_a_failure(self):
        for shell in SHELLS:
            with self.subTest(shell=shell):
                done = self.run_script(shell, GETBOOT="/dev/disk1s1")
                self.assertNotEqual(done.returncode, 0)
                self.assertIn(f"starts /dev/disk1s1, not {NODE}", done.stderr)

    def test_no_loader_no_bless(self):
        (self.volume / "EFI" / "BOOT" / "BOOTX64.EFI").unlink()
        for shell in SHELLS:
            with self.subTest(shell=shell):
                done = self.run_script(shell)
                self.assertNotEqual(done.returncode, 0)
                self.assertIn("no EFI/BOOT/BOOTX64.EFI", done.stderr)
                self.assertEqual(self.bless_calls(), [])

    def test_volume_unknown_to_diskutil(self):
        for shell in SHELLS:
            with self.subTest(shell=shell):
                done = self.run_script(shell, VOLUME="/Volumes/OTHER")
                self.assertNotEqual(done.returncode, 0)
                self.assertIn("diskutil does not know", done.stderr)
                self.assertEqual(self.bless_calls(), [])


if __name__ == "__main__":
    unittest.main()
