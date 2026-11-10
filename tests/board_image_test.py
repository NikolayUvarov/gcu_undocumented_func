"""board_image.py on an image made by make_usb_image.py's own partition writers: the partitions moved past U-Boot, their
volumes still read whole, U-Boot at 8 KiB, and the refusals (100004-MOB-0001). Only files in a temporary directory."""
from pathlib import Path
import struct
import sys
import tempfile
import time
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from scripts import board_image  # noqa: E402
from scripts.make_usb_image import (LOG_README, LOG_SECTORS, SECTOR, add_log_partition, add_store_partition,  # noqa: E402
                                    check_image, log_volume)

VVFAT_START = 63  # where make_usb_image.py's vvfat puts the boot partition
PAYLOADS = {"README.TXT": LOG_README}  # the stand-in boot volume is a log volume: its README is its one file


def image(path):
    """An image laid out as make_usb_image.py lays one out: a FAT16 boot partition at 63, then the log and store."""
    mbr = bytearray(SECTOR)
    struct.pack_into("<B3sB3sII", mbr, 446, 0x80, b"\xfe\xff\xff", 0xEF, b"\xfe\xff\xff", VVFAT_START, LOG_SECTORS)
    mbr[510:] = b"\x55\xaa"
    with path.open("wb") as disk:
        disk.write(mbr)
        disk.seek(VVFAT_START * SECTOR)
        disk.write(log_volume(VVFAT_START, time.localtime()))
    add_log_partition(path, time.localtime())
    add_store_partition(path)
    check_image(path, PAYLOADS)
    return path


def uboot(path, size=600 * 1024):
    blob = bytearray(size)
    blob[4:12] = board_image.MAGIC
    for i in range(32, size):
        blob[i] = (i * 7 + 3) & 0xFF
    path.write_bytes(bytes(blob))
    return path


def entries(path):
    with path.open("rb") as disk:
        return board_image.partitions(disk.read(SECTOR))


class BoardImage(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory(prefix="board-image-")
        self.root = Path(self.dir.name)
        self.image = image(self.root / "mind.img")

    def tearDown(self):
        self.dir.cleanup()

    def test_partitions_move_past_uboot(self):
        card = self.root / "card.img"
        board_image.make(self.image, uboot(self.root / "u-boot.bin"), card)
        before, after = entries(self.image), entries(card)
        self.assertEqual([k for k, _, _ in after], [0xEF, 0x0E, 0x7F])
        self.assertEqual([n for _, _, n in after], [n for _, _, n in before])
        boot, log, store = (start for _, start, _ in after)
        self.assertEqual(boot, board_image.BOOT_START)
        self.assertEqual(log % 2048, 0)
        self.assertEqual(store % 2048, 0)
        # Each volume reads back whole where it moved to; the FATs name their new start.
        check_image(card, PAYLOADS)
        with self.image.open("rb") as old, card.open("rb") as new:
            for (_, was, sectors), (_, now, _) in zip(before, after):
                old.seek(was * SECTOR)
                new.seek(now * SECTOR)
                first_old, first_new = bytearray(old.read(SECTOR)), bytearray(new.read(SECTOR))
                if first_old[510:] == b"\x55\xaa" and first_old[54:59] == b"FAT16":
                    self.assertEqual(struct.unpack_from("<I", first_new, 28)[0], now)
                    struct.pack_into("<I", first_old, 28, now)
                self.assertEqual(first_new, first_old)
                self.assertEqual(new.read((sectors - 1) * SECTOR), old.read((sectors - 1) * SECTOR))

    def test_uboot_at_8_kib_and_the_mbr_kept(self):
        card = self.root / "card.img"
        blob = uboot(self.root / "u-boot.bin").read_bytes()
        board_image.make(self.image, self.root / "u-boot.bin", card)
        with card.open("rb") as disk:
            mbr = disk.read(SECTOR)
            disk.seek(board_image.SPL_OFFSET)
            self.assertEqual(disk.read(len(blob)), blob)
        with self.image.open("rb") as disk:
            old = disk.read(SECTOR)
        self.assertEqual(mbr[:446], old[:446])  # the boot code and the disk's signature
        self.assertEqual(mbr[510:], b"\x55\xaa")

    def test_refusals(self):
        card = self.root / "card.img"
        plain = self.root / "plain.bin"
        plain.write_bytes(bytes(4096))
        with self.assertRaisesRegex(ValueError, "eGON.BT0"):
            board_image.make(self.image, plain, card)
        big = uboot(self.root / "big.bin", size=board_image.BOOT_START * SECTOR)
        with self.assertRaisesRegex(ValueError, "past the first partition"):
            board_image.make(self.image, big, card)
        small = uboot(self.root / "u-boot.bin")
        board_image.make(self.image, small, card)
        with self.assertRaisesRegex(ValueError, "already exists"):
            board_image.make(self.image, small, card)
        board_image.make(self.image, small, card, force=True)
        two = self.root / "two.img"
        with self.image.open("rb") as disk:
            data = bytearray(disk.read(SECTOR))
        data[478:494] = bytes(16)  # no store partition
        two.write_bytes(bytes(data))
        with self.assertRaisesRegex(ValueError, "boot, log and store"):
            board_image.make(two, small, self.root / "out.img")
        self.assertFalse((self.root / "out.img").exists())


if __name__ == "__main__":
    unittest.main()
