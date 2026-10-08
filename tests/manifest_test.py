#!/usr/bin/env python3
"""Tests of boot volume signing (scripts/sign_manifest.py, issue 350-UPD-0002): Ed25519 against RFC 8032's vectors and,
where installed, the `cryptography` library; the manifest's lines; a changed file, manifest or signature refused; the
requests a program declares; the test key built into the bootloader."""
import hashlib
import os
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
import sign_manifest as sm  # noqa: E402

# RFC 8032, section 7.1, tests 1 to 3: seed, public key, message, signature.
VECTORS = [
    ("9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60", "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a", "",
     "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e065224901555fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b"),
    ("4ccd089b28ff96da9db6c346ec114e0f5b8a319f35aba624da8cf6ed4fb8a6fb", "3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c", "72",
     "92a009a9f0d4cab8720e820b5f642540a2b27b5416503f8fb3762223ebdb69da085ac1e43e15996e458f3613d0f11d8c387b2eaeb4302aeeb00d291612bb0c00"),
    ("c5aa8df43f9f837bedb7442f31dcb7b166d38535076f094b85ce3a2e0b4458f7", "fc51cd8e6218a1a38da47ed00230f0580816ed13ba3303ac5deb911548908025", "af82",
     "6291d657deec24024827e69c3abe01a30ce548a284743a445e3680d7db5ac3ac18ff9b538d16f290ae67f760984dc6594a7c15e9716ed28dc027beceea1ec40a"),
]


class Ed25519(unittest.TestCase):
    def test_rfc_8032_vectors(self):
        for seed, public, message, signature in VECTORS:
            seed, public, message, signature = map(bytes.fromhex, (seed, public, message, signature))
            self.assertEqual(sm.public_key(seed), public)
            self.assertEqual(sm.sign(seed, message), signature)
            self.assertTrue(sm.verify(public, message, signature))
            self.assertFalse(sm.verify(public, message + b"x", signature))
            self.assertFalse(sm.verify(public, message, signature[:63] + bytes([signature[63] ^ 1])))

    def test_a_scalar_past_the_group_order_is_refused(self):
        seed, public, message, signature = map(bytes.fromhex, VECTORS[0])
        s = int.from_bytes(signature[32:], "little") + sm.L
        self.assertFalse(sm.verify(public, message, signature[:32] + s.to_bytes(32, "little")))

    def test_the_same_as_an_independent_library(self):
        try:
            from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
        except ImportError:
            self.skipTest("cryptography is not installed")
        for n in range(8):
            seed, message = hashlib.sha256(bytes([n])).digest(), os.urandom(n * 61)
            self.assertEqual(sm.sign(seed, message), Ed25519PrivateKey.from_private_bytes(seed).sign(message))


class Manifest(unittest.TestCase):
    def volume(self, temp):
        v = Path(temp)
        (v / "EFI/BOOT").mkdir(parents=True)
        (v / "EFI/BOOT/BOOTX64.EFI").write_bytes(b"MZ loader")
        (v / "kernel.elf").write_bytes(b"\x7fELF kernel")
        (v / "LICENSES").mkdir()
        (v / "LICENSES/LICENSE-MIT").write_bytes(b"licence")
        (v / "scratch.img").write_bytes(b"not shipped")
        return v

    def test_lines_and_a_round_trip(self):
        with tempfile.TemporaryDirectory() as temp:
            v = self.volume(temp)
            text = sm.sign_volume(v, sm.TEST_SEED, ("c" * 40, "nightly-x", "d" * 64)).decode()
            lines = text.splitlines()
            self.assertEqual(lines[:5], ["MIND-MANIFEST 1", f"key {hashlib.sha256(sm.public_key(sm.TEST_SEED)).hexdigest()[:16]}", "commit " + "c" * 40, "toolchain nightly-x", "inputs " + "d" * 64])
            # Shipped files only, sorted; a file nobody ships is not listed.
            self.assertEqual([l.split(" ")[1] for l in lines[5:]], ["EFI/BOOT/BOOTX64.EFI", "LICENSES/LICENSE-MIT", "kernel.elf"])
            kernel = hashlib.sha256(b"\x7fELF kernel").hexdigest()
            self.assertIn(f"file kernel.elf 11 {kernel} 00000000 0", lines)
            self.assertIsNone(sm.verify_volume(v, sm.public_key(sm.TEST_SEED)))

    def test_a_change_is_refused(self):
        public = sm.public_key(sm.TEST_SEED)
        with tempfile.TemporaryDirectory() as temp:
            v = self.volume(temp)
            sm.sign_volume(v, sm.TEST_SEED)
            (v / "kernel.elf").write_bytes(b"\x7fELF kernel!")
            self.assertEqual(sm.verify_volume(v, public), "kernel.elf: not as the manifest says")
            sm.sign_volume(v, sm.TEST_SEED)
            (v / "MANIFEST").write_bytes((v / "MANIFEST").read_bytes().replace(b"toolchain", b"toolchaim"))
            self.assertEqual(sm.verify_volume(v, public), "MANIFEST: bad signature")
            # Signed with another key: refused by a bootloader that holds the test key.
            sm.sign_volume(v, hashlib.sha256(b"another").digest())
            self.assertEqual(sm.verify_volume(v, public), "MANIFEST: bad signature")

    def test_requests_of_a_program(self):
        # A program built with mind::request!: its flags and memory, as the loader reads them.
        blocks = ROOT / "usb_root" / "blocks.elf"
        if not blocks.exists():
            self.skipTest("usb_root is not built")
        flags, memory = sm.requests(blocks.read_bytes())
        self.assertNotEqual(flags, 0)
        self.assertEqual(sm.requests(b"not an ELF"), (0, 0))

    def test_the_bootloaders_test_key(self):
        self.assertEqual((ROOT / "bootloader/keys/test.pub").read_text().strip(), sm.public_key(sm.TEST_SEED).hex())


if __name__ == "__main__":
    unittest.main(verbosity=1)
