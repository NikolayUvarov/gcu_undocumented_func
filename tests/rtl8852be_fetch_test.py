#!/usr/bin/env python3
"""Offline verification of the pinned firmware fetcher's corruption boundaries."""

import hashlib
import importlib.util
import io
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.dont_write_bytecode = True
SPEC = importlib.util.spec_from_file_location(
    "rtl8852be", Path(__file__).resolve().parents[1] / "scripts/firmware/rtl8852be.py")
firmware = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(firmware)


class FetchTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.directory = Path(self.temp.name)
        self.data = b"synthetic firmware fixture"
        self.asset = ("test.bin", "test.bin", len(self.data), hashlib.sha256(self.data).hexdigest())

    def test_existing_verified_file_needs_no_network(self):
        path = self.directory / "test.bin"
        path.write_bytes(self.data)
        with patch.object(firmware.urllib.request, "urlopen") as request:
            self.assertEqual(firmware.ensure_asset(self.directory, self.asset, True), path)
            request.assert_not_called()

    def test_check_never_downloads_missing_or_corrupt_data(self):
        with patch.object(firmware.urllib.request, "urlopen") as request:
            with self.assertRaises(FileNotFoundError):
                firmware.ensure_asset(self.directory, self.asset, False)
            (self.directory / "test.bin").write_bytes(b"x" * len(self.data))
            with self.assertRaisesRegex(ValueError, "SHA-256"):
                firmware.ensure_asset(self.directory, self.asset, False)
            request.assert_not_called()

    def test_invalid_download_never_replaces_existing_file(self):
        path = self.directory / "test.bin"
        path.write_bytes(b"old invalid cache")
        for data in [self.data[:-1], self.data + b"extra", b"x" * len(self.data)]:
            with patch.object(firmware.urllib.request, "urlopen", return_value=io.BytesIO(data)):
                with self.assertRaises(ValueError):
                    firmware.ensure_asset(self.directory, self.asset, True)
            self.assertEqual(path.read_bytes(), b"old invalid cache")
            self.assertEqual(list(self.directory.iterdir()), [path])

    def test_verified_download_is_stored_atomically(self):
        with patch.object(firmware.urllib.request, "urlopen", return_value=io.BytesIO(self.data)):
            path = firmware.ensure_asset(self.directory, self.asset, True)
        self.assertEqual(path.read_bytes(), self.data)
        self.assertEqual(list(self.directory.iterdir()), [path])

    def test_response_read_is_bounded(self):
        response = io.BytesIO(self.data + bytes(100000))
        with patch.object(firmware.urllib.request, "urlopen", return_value=response):
            with patch.object(response, "read", wraps=response.read) as read:
                with self.assertRaises(ValueError):
                    firmware.ensure_asset(self.directory, self.asset, True)
                read.assert_called_once_with(len(self.data) + 1)


if __name__ == "__main__":
    unittest.main()
