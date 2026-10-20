#!/usr/bin/env python3
"""Tests of the signing tool for application packages (scripts/app_package.py, 750-UPD-0019, contract draft 0.1): it
builds apppkg/tests/fixture's MANIFEST and MANIFEST.SIG again byte for byte (the device's check, `apppkg`, verifies
that fixture in its own tests); its checker refuses a changed file, a changed manifest, another key, a signature
without the package prefix, another architecture and a release too old."""
import shutil
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
import app_package as ap  # noqa: E402
import sign_manifest as sm  # noqa: E402

FIXTURE = ROOT / "apppkg/tests/fixture"
PUBLIC = sm.public_key(ap.TEST_SEED)


class AppPackage(unittest.TestCase):
    def copy(self, where):
        target = Path(where) / "pkg"
        shutil.copytree(FIXTURE, target)
        return target

    def test_the_fixture_is_built_again_byte_for_byte(self):
        with tempfile.TemporaryDirectory() as where:
            pkg = self.copy(where)
            (pkg / "MANIFEST").unlink()
            (pkg / "MANIFEST.SIG").unlink()
            ap.build(pkg, "hello", 3, "x86_64", 7, ap.TEST_SEED)
            self.assertEqual((pkg / "MANIFEST").read_bytes(), (FIXTURE / "MANIFEST").read_bytes())
            self.assertEqual((pkg / "MANIFEST.SIG").read_bytes(), (FIXTURE / "MANIFEST.SIG").read_bytes())
            self.assertIsNone(ap.check(pkg, PUBLIC, "x86_64", 7))

    def test_the_checker_refuses_what_the_device_refuses(self):
        with tempfile.TemporaryDirectory() as where:
            pkg = self.copy(where)
            self.assertEqual(ap.check(pkg, PUBLIC, "aarch64", 7), "ANOTHER ARCHITECTURE")
            self.assertEqual(ap.check(pkg, PUBLIC, "x86_64", 6), "NEEDS A NEWER RELEASE")
            self.assertEqual(ap.check(pkg, sm.public_key(b"\7" * 32)), "UNTRUSTED: another key")
            (pkg / "data/greeting.txt").write_bytes(b"changed\n")
            self.assertEqual(ap.check(pkg, PUBLIC), "CHANGED: data/greeting.txt")
            shutil.copy(FIXTURE / "data/greeting.txt", pkg / "data/greeting.txt")
            text = (pkg / "MANIFEST").read_bytes()
            (pkg / "MANIFEST").write_bytes(text.replace(b"version 3", b"version 4"))
            self.assertEqual(ap.check(pkg, PUBLIC), "SIGNATURE")
            # A signature over the manifest alone, as a boot manifest's is, does not pass.
            (pkg / "MANIFEST").write_bytes(text)
            (pkg / "MANIFEST.SIG").write_bytes(sm.sign(ap.TEST_SEED, text))
            self.assertEqual(ap.check(pkg, PUBLIC), "SIGNATURE")

    def test_build_refuses_what_the_format_does_not_hold(self):
        with tempfile.TemporaryDirectory() as where:
            pkg = self.copy(where)
            for name, version, arch, system in (("Hello", 3, "x86_64", 0), ("hello", 0, "x86_64", 0), ("hello", 3, "x86", 0)):
                with self.assertRaises(ValueError):
                    ap.build(pkg, name, version, arch, system, ap.TEST_SEED)
            (pkg / "Upper.txt").write_bytes(b"x")
            with self.assertRaises(ValueError):
                ap.build(pkg, "hello", 3, "x86_64", 0, ap.TEST_SEED)


if __name__ == "__main__":
    unittest.main()
