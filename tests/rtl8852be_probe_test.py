#!/usr/bin/env python3
"""RTL8852BE identity, privacy and strictly read-only collection boundaries."""

import contextlib
import importlib.util
import io
import json
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

sys.dont_write_bytecode = True
SPEC = importlib.util.spec_from_file_location(
    "rtl8852be_probe", Path(__file__).resolve().parents[1] / "scripts/hardware/rtl8852be_probe.py")
probe = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(probe)

BUS = "0000:03:00.0"
RFE = f"rtw89_8852be {BUS}: chip rfe_type is 1"
FW = f"rtw89_8852be {BUS}: Firmware version 0.29.29.15 (6fb3ec41), cmd version 0, type 5"


class RecordTests(unittest.TestCase):
    def test_only_known_numeric_messages_from_the_exact_device_survive(self):
        text = "\n".join([RFE, FW, RFE, "wlan0: SSID private-network password=secret",
                           RFE.replace(BUS, "0000:04:00.0"), RFE + " SSID private-network",
                           "another-driver said " + RFE])
        records = probe.parse_boot_records(text, BUS)
        self.assertEqual(records, [{"kind": "rfe_type", "value": 1},
                                  {"kind": "firmware", "version": "0.29.29.15", "commit": "6fb3ec41",
                                   "command_version": 0, "type": 5}])
        self.assertNotIn("private-network", json.dumps(records))

    def test_malformed_and_out_of_range_values_are_not_evidence(self):
        for text in [RFE.replace("is 1", "is 256"), RFE.replace("is 1", "is -1"),
                     FW.replace("0.29.29.15", "0.29.999.15"), FW.replace("6fb3ec41", "not-a-hash"),
                     FW.replace("type 5", "type 256"), FW.replace("0.29.29.15", "1.2.3")]:
            self.assertEqual(probe.parse_boot_records(text, BUS), [])
        with self.assertRaises(probe.ProbeError):
            probe.parse_boot_records("x" * (probe.MAX_TEXT + 1), BUS)

    def test_journal_filter_is_anchored_and_excludes_other_devices_and_messages(self):
        pattern = re.compile(probe.log_pattern("03:00.0"))
        self.assertRegex(RFE, pattern)
        self.assertRegex(FW, pattern)
        for text in [RFE.replace(BUS, "0000:04:00.0"), "SSID=" + RFE, RFE + " secret"]:
            self.assertIsNone(pattern.search(text))

    def test_driver_info_ignores_unrelated_fields_and_rejects_duplicates(self):
        info = probe.driver_info("driver: rtw89_8852be\nSSID: private-network\nbus-info: 0000:03:00.0\n")
        self.assertEqual(info, {"driver": "rtw89_8852be", "bus-info": BUS})
        with self.assertRaises(probe.ProbeError):
            probe.driver_info("driver: rtw89_8852be\ndriver: other\n")


class CollectionTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.sysfs = self.root / "devices"
        self.device = self.sysfs / BUS
        self.device.mkdir(parents=True)
        for name, value in {"vendor": "0x10ec", "device": "0xb852", "revision": "0x00",
                            "subsystem_vendor": "0x10ec", "subsystem_device": "0xb852",
                            "class": "0x028000", "irq": "55"}.items():
            (self.device / name).write_text(value + "\n")
        (self.device / "resource").write_text("00000000fcd00000 00000000fcdfffff 0000000000140204\n")
        (self.device / "resource2").write_bytes(b"MMIO must never be opened")
        self.driver = self.root / probe.DRIVER
        self.driver.mkdir()
        (self.device / "driver").symlink_to(self.driver)
        self.interface = self.device / "net" / "wlp3s0"
        self.interface.mkdir(parents=True)
        for name, value in {"operstate": "up", "carrier": "1", "mtu": "1500"}.items():
            (self.interface / name).write_text(value + "\n")
        self.calls = []
        self.journal = (RFE + "\n" + FW + "\nSSID private-network password=secret\n").encode()
        self.info_bus = BUS

    def query(self, argv):
        self.calls.append(argv)
        if argv == ["/usr/sbin/ethtool", "-i", "wlp3s0"]:
            data = f"driver: rtw89_8852be\nversion: 6.19\nfirmware-version: N/A\nbus-info: {self.info_bus}\nSSID: private-network\n".encode()
        elif argv == ["/usr/bin/journalctl", "--kernel", "--boot=0", "--no-pager", "--output=cat",
                      "--lines=32", "--grep=" + probe.log_pattern(BUS)]:
            data = self.journal
        else:
            self.fail(f"Unexpected command: {argv}")
        return data, {"argv": argv, "returncode": 0, "stderr": ""}

    def collect(self, name="report", ethtool="/usr/sbin/ethtool", journalctl="/usr/bin/journalctl"):
        output = self.root / name
        output.mkdir(mode=0o700)
        with mock.patch.object(probe, "query", side_effect=self.query):
            report = probe.collect(self.device, output, ethtool, journalctl)
        return report, output

    def test_collects_identity_with_no_mmio_debugfs_or_network_configuration_reads(self):
        opened = []
        original = Path.open

        def tracked(path, *args, **kwargs):
            opened.append(path)
            return original(path, *args, **kwargs)

        before = {str(p): p.read_bytes() for p in self.device.rglob("*") if p.is_file()}
        with mock.patch.object(Path, "open", tracked):
            report, output = self.collect()
        self.assertEqual(report["rfe_type"], 1)
        self.assertEqual(report["chip_cut"], None)
        self.assertEqual(report["status"], "partial")
        self.assertIn("SYS_CFG1", report["chip_cut_missing_reason"])
        self.assertEqual(len(self.calls), 2)
        self.assertNotIn(self.device / "resource2", opened)
        self.assertTrue(all("debugfs" not in str(path) for path in opened))
        self.assertNotIn("private-network", (output / "report.json").read_text())
        self.assertNotIn("secret", (output / "report.json").read_text())
        self.assertEqual(before, {str(p): p.read_bytes() for p in self.device.rglob("*") if p.is_file()})
        self.assertEqual(output.stat().st_mode & 0o777, 0o700)
        self.assertEqual((output / "report.json").stat().st_mode & 0o777, 0o600)
        self.assertEqual(list(output.iterdir()), [output / "report.json"])

    def test_wrong_or_ambiguous_device_is_refused_before_creating_report_or_querying(self):
        output = self.root / "refused"
        for value in ["0x8168", "0x8852"]:
            (self.device / "device").write_text(value)
            with mock.patch.object(probe, "SYSFS", self.sysfs), mock.patch.object(probe, "query") as query:
                with contextlib.redirect_stderr(io.StringIO()):
                    self.assertEqual(probe.main(["--pci", BUS, "--output", str(output)]), 2)
                query.assert_not_called()
                self.assertFalse(output.exists())
        (self.device / "device").write_text("0xb852")
        other = self.sysfs / "0000:04:00.0"
        other.mkdir()
        (other / "vendor").write_text("0x10ec")
        (other / "device").write_text("0xb852")
        with self.assertRaises(probe.ProbeError):
            probe.select_device(self.sysfs, None)

    def test_missing_driver_or_tools_is_partial_without_any_fallback_access(self):
        report, _ = self.collect(ethtool=None, journalctl=None)
        self.assertEqual(self.calls, [])
        self.assertIsNone(report["rfe_type"])
        self.assertTrue(any("Missing journalctl" in warning for warning in report["warnings"]))
        (self.device / "driver").unlink()
        report, _ = self.collect("no-driver")
        self.assertEqual(self.calls, [])
        self.assertIsNone(report["driver"])

    def test_mismatched_bus_and_conflicting_rfe_do_not_become_hardware_facts(self):
        self.info_bus = "0000:04:00.0"
        self.journal += (RFE.replace("is 1", "is 2") + "\n").encode()
        report, _ = self.collect()
        self.assertNotIn("driver_info", report["interfaces"][0])
        self.assertIsNone(report["rfe_type"])
        self.assertTrue(any("Conflicting RFE" in warning for warning in report["warnings"]))

    def test_existing_output_is_preserved(self):
        output = self.root / "existing"
        output.mkdir()
        (output / "keep").write_text("keep")
        with mock.patch.object(probe, "SYSFS", self.sysfs), mock.patch.object(probe, "query") as query:
            with contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(probe.main(["--pci", BUS, "--output", str(output)]), 2)
            query.assert_not_called()
        self.assertEqual((output / "keep").read_text(), "keep")

    def test_permission_timeout_and_oversized_output_are_reported_without_escalation(self):
        argv = ["journalctl", "--kernel"]
        denied = subprocess.CompletedProcess(argv, 1, b"", b"Operation not permitted")
        with mock.patch.object(probe.subprocess, "run", return_value=denied) as run:
            _, status = probe.query(argv)
        self.assertIn("administrative read access", status["stderr"])
        self.assertNotIn("shell", run.call_args.kwargs)
        with mock.patch.object(probe.subprocess, "run", side_effect=subprocess.TimeoutExpired(argv, 10)):
            data, status = probe.query(argv)
            self.assertEqual(data, b"")
            self.assertIsNone(status["returncode"])
        oversized = subprocess.CompletedProcess(argv, 0, bytes(probe.MAX_TEXT + 1), b"")
        with mock.patch.object(probe.subprocess, "run", return_value=oversized):
            data, status = probe.query(argv)
            self.assertEqual(data, b"")
            self.assertIn("Oversized", status["stderr"])


if __name__ == "__main__":
    unittest.main()
