#!/usr/bin/env python3
"""RTL8168 evidence parser, refusal paths and read-only query contract (213-DRV-0030)."""

import contextlib
import importlib.util
import io
import json
from pathlib import Path
import tempfile
import subprocess
import sys
import unittest
from unittest import mock


SCRIPT = Path(__file__).resolve().parents[1] / "scripts/hardware/rtl8168_probe.py"
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("rtl8168_probe", SCRIPT)
probe = importlib.util.module_from_spec(spec)
spec.loader.exec_module(probe)


def dump(txconfig=b"\x80\x07\x00\x4c"):
    raw = bytearray(256)
    raw[0x3C:0x40] = b"\xff" * 4
    raw[0x40:0x44] = txconfig
    raw[0x44:0x48] = b"\xff" * 4
    return bytes(raw)


class RegisterTests(unittest.TestCase):
    def decode(self, raw, driver="r8169", bus="0000:02:00.0", endian="little"):
        return probe.decode_registers(raw, driver, bus, "0000:02:00.0", endian)

    def test_known_register_values_decode_at_the_correct_offset_and_endianness(self):
        for raw, txconfig, xid in [
            (b"\x80\x07\x00\x4c", "0x4c000780", "0x4c0"),
            (b"\x80\x07\x10\x54", "0x54100780", "0x541"),
            (b"\x07\xa0\xf2\x4f", "0x4ff2a007", "0x4cf"),
        ]:
            with self.subTest(txconfig=txconfig):
                result = self.decode(dump(raw))
                self.assertEqual((result["txconfig"], result["xid"]), (txconfig, xid))

    def test_invalid_inaccessible_truncated_or_unknown_layouts_are_refused(self):
        for raw in [dump()[:0x43], dump()[:-1], dump() + b"\0", dump(b"\xff" * 4), dump(b"\0" * 4), dump(b"\x80\x07\0\0")]:
            with self.subTest(length=len(raw)):
                with self.assertRaises(probe.ProbeError):
                    self.decode(raw)
        for kwargs in [{"driver": "r8168"}, {"driver": "e1000e"}, {"bus": "0000:03:00.0"}, {"bus": ""}, {"endian": "big"}]:
            with self.subTest(kwargs=kwargs):
                with self.assertRaises(probe.ProbeError):
                    self.decode(dump(), **kwargs)


class CollectionTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.sysfs = self.root / "devices"
        self.sysfs.mkdir()
        self.device = self.sysfs / "0000:02:00.0"
        self.device.mkdir()
        for name, value in {"vendor": "0x10ec", "device": "0x8168", "revision": "0x0c", "subsystem_vendor": "0x10ec", "subsystem_device": "0x0123", "class": "0x020000", "irq": "45"}.items():
            (self.device / name).write_text(value + "\n")
        (self.device / "resource").write_text("000000000000f000 000000000000f0ff 0000000000000101\n0000000000000000 0000000000000000 0000000000000000\n00000000fce00000 00000000fce00fff 0000000000140204\n")
        self.driver = self.root / "r8169"
        self.driver.mkdir()
        (self.device / "driver").symlink_to(self.driver)
        self.interface = self.device / "net" / "enp2s0"
        self.interface.mkdir(parents=True)
        for name, value in {"address": "02:00:00:00:00:01", "operstate": "up", "carrier": "1", "speed": "1000", "duplex": "full", "mtu": "1500"}.items():
            (self.interface / name).write_text(value + "\n")

    def snapshot(self):
        return {str(path.relative_to(self.sysfs)): path.read_bytes() for path in self.sysfs.rglob("*") if path.is_file()}

    def test_wrong_vendor_or_device_is_refused_before_output_or_hardware_tools(self):
        for attr, value in [("vendor", "0x8086"), ("device", "0x8125")]:
            original = (self.device / attr).read_text()
            (self.device / attr).write_text(value)
            output = self.root / "refused"
            with mock.patch.object(probe, "SYSFS", self.sysfs), mock.patch.object(probe, "query") as query, contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(probe.main(["--pci", self.device.name, "--output", str(output)]), 2)
                query.assert_not_called()
                self.assertFalse(output.exists())
            (self.device / attr).write_text(original)

    def test_collection_preserves_sysfs_and_uses_only_information_queries(self):
        output = self.root / "report"
        before = self.snapshot()
        called = []

        def query(argv):
            called.append(argv)
            if argv == ["/usr/sbin/ethtool", "-i", "enp2s0"]:
                data = b"driver: r8169\nversion: 6.12\nfirmware-version: fixture\nbus-info: 0000:02:00.0\n"
            elif argv == ["/usr/sbin/ethtool", "enp2s0"]:
                data = b"Settings for enp2s0:\nLink detected: yes\n"
            elif argv == ["/usr/sbin/ethtool", "-d", "enp2s0", "raw", "on"]:
                data = dump()
            else:
                self.fail(f"Unexpected hardware command: {argv}")
            return data, {"argv": argv, "returncode": 0, "stderr": ""}

        with mock.patch.object(probe, "SYSFS", self.sysfs), mock.patch.object(probe.shutil, "which", return_value="/usr/sbin/ethtool"), mock.patch.object(probe, "query", side_effect=query), contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(probe.main(["--output", str(output)]), 0)
        self.assertEqual(self.snapshot(), before)
        self.assertEqual(len(called), 3)
        report = json.loads((output / "report.json").read_text())
        self.assertEqual(report["interfaces"][0]["registers"]["xid"], "0x4c0")
        self.assertEqual(report["resources"][2]["size"], 4096)
        self.assertEqual((output / "enp2s0-registers.bin").read_bytes(), dump())
        self.assertEqual(output.stat().st_mode & 0o777, 0o700)
        self.assertEqual((output / "report.json").stat().st_mode & 0o777, 0o600)

    def test_missing_ethtool_produces_an_actionable_partial_report(self):
        output = self.root / "partial"
        with mock.patch.object(probe, "SYSFS", self.sysfs), mock.patch.object(probe.shutil, "which", return_value=None), mock.patch.object(probe, "query") as query, contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            self.assertEqual(probe.main(["--output", str(output)]), 1)
            query.assert_not_called()
        report = json.loads((output / "report.json").read_text())
        self.assertEqual(report["status"], "partial")
        self.assertTrue(any("Missing ethtool" in item for item in report["warnings"]))

    def test_ambiguous_selection_and_existing_output_are_refused(self):
        second = self.sysfs / "0000:04:00.0"
        second.mkdir()
        (second / "vendor").write_text("0x10ec")
        (second / "device").write_text("0x8168")
        with self.assertRaises(probe.ProbeError):
            probe.select_device(self.sysfs, None)
        output = self.root / "existing"
        output.mkdir()
        (output / "keep").write_text("keep")
        with mock.patch.object(probe, "SYSFS", self.sysfs), mock.patch.object(probe, "query") as query, contextlib.redirect_stderr(io.StringIO()):
            self.assertEqual(probe.main(["--pci", self.device.name, "--output", str(output)]), 2)
            query.assert_not_called()
        self.assertEqual((output / "keep").read_text(), "keep")

    def test_permission_failure_names_the_required_read_privilege_without_escalating(self):
        argv = ["ethtool", "-d", "enp2s0", "raw", "on"]
        denied = subprocess.CompletedProcess(argv, 1, b"", b"Cannot get register dump: Operation not permitted")
        with mock.patch.object(probe.subprocess, "run", return_value=denied) as run:
            data, result = probe.query(argv)
        self.assertEqual(data, b"")
        self.assertEqual(result["returncode"], 1)
        self.assertIn("administrative read access may be required", result["stderr"])
        self.assertEqual(run.call_args.args[0], argv)
        self.assertNotIn("shell", run.call_args.kwargs)


if __name__ == "__main__":
    unittest.main()
