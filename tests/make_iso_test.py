"""make_iso.py on stand-in payloads (352-UPD-0047): the ISO's descriptors, catalog, root directory and MBR name one boot
volume that holds every payload; the same date gives the same bytes; a changed byte where they meet is refused. Only files
in a temporary directory; needs mkfs.fat and mtools."""
import os
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from scripts import make_iso  # noqa: E402

WHEN = 1763668800  # 2025-11-20 20:00 UTC
PAYLOADS = {"EFI/BOOT/BOOTX64.EFI": b"MZ" + bytes(4094), "kernel.elf": b"\x7fELF" + bytes(70000),
            "dzen-clock.elf": b"\x7fELF" + bytes(10), "LICENSES/THIRD_PARTY.md": b"third party\n", "MANIFEST": b"m\n"}
TOOLS = all(shutil.which(t) for t in ("mkfs.fat", "mcopy", "mtype"))


def build(directory, payloads=PAYLOADS, when=WHEN):
    volume, iso = directory / "boot.img", directory / "out.iso"
    make_iso.boot_volume(payloads, volume, when)
    return iso, make_iso.write_iso(volume, iso, when, "x86_64")


@unittest.skipUnless(TOOLS, "needs mkfs.fat and mtools")
class MakeIso(unittest.TestCase):
    def test_one_volume_holds_every_payload(self):
        with tempfile.TemporaryDirectory() as d:
            iso, _ = build(Path(d))
            lba, size = make_iso.check_iso(iso)
            self.assertEqual(lba, make_iso.VOLUME_LBA)
            self.assertEqual(lba * make_iso.BLOCK + size, iso.stat().st_size)
            env = dict(os.environ, MTOOLS_SKIP_CHECK="1")
            for name, data in PAYLOADS.items():
                out = subprocess.run(["mtype", "-i", f"{iso}@@{lba * make_iso.BLOCK}", f"::{name}"], capture_output=True, env=env)
                self.assertEqual(out.stdout, data, name)
            listing = subprocess.run(["mdir", "-i", f"{iso}@@{lba * make_iso.BLOCK}", "::data"], capture_output=True, text=True, env=env)
            self.assertIn("machine", listing.stdout)

    def test_the_same_date_gives_the_same_bytes(self):
        with tempfile.TemporaryDirectory() as a, tempfile.TemporaryDirectory() as b:
            self.assertEqual(build(Path(a))[1], build(Path(b))[1])
            self.assertNotEqual(build(Path(a), when=WHEN + 60)[1], build(Path(b))[1])

    def test_a_volume_over_32_mib_is_counted_to_the_end(self):
        with tempfile.TemporaryDirectory() as d:
            iso, _ = build(Path(d))
            data = iso.read_bytes()
            count = struct.unpack_from("<H", data, make_iso.CATALOG * make_iso.BLOCK + 38)[0]
            self.assertGreater(make_iso.MIN_MB << 20, 0xFFFF * make_iso.SECTOR)
            self.assertEqual(count, 1)

    def test_a_changed_byte_is_refused(self):
        places = {
            "the MBR's partition start": 446 + 8,
            "the MBR's type": 446 + 4,
            "the volume's size": make_iso.PVD * make_iso.BLOCK + 80,
            "the boot record's catalog": make_iso.BOOT_RECORD * make_iso.BLOCK + 71,
            "the catalog's checksum": make_iso.CATALOG * make_iso.BLOCK + 28,
            "the boot entry's place": make_iso.CATALOG * make_iso.BLOCK + 40,
            "the volume's listed extent": make_iso.ROOT_DIR * make_iso.BLOCK + 2 * 34 + 44 + 2,  # after ".", ".." and BOOT.CAT;1
            "the volume's hidden sectors": make_iso.VOLUME_LBA * make_iso.BLOCK + 28,
        }
        with tempfile.TemporaryDirectory() as d:
            iso, _ = build(Path(d))
            good = iso.read_bytes()
            for what, offset in places.items():
                bad = bytearray(good)
                bad[offset] ^= 0x01
                iso.write_bytes(bytes(bad))
                with self.assertRaises(ValueError, msg=what):
                    make_iso.check_iso(iso)
            iso.write_bytes(good + bytes(make_iso.BLOCK))
            with self.assertRaises(ValueError, msg="a block past the volume"):
                make_iso.check_iso(iso)


if __name__ == "__main__":
    unittest.main()
