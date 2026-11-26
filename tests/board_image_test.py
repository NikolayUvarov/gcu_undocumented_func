"""board_image.py and its catalog (100004-MOB-0001, 352-MOB-0010): each catalog entry's card from an image laid out as a
local build's and as a release's, and from a boot volume alone; the partitions moved past the firmware, their volumes still
read whole, the firmware at the entry's offset; the same card from the same volume; the refusals; the catalog's entries,
THIRD_PARTY.md's record of their sources, and board_uboot.sh's reading of an entry. Only files in a temporary directory."""
import os
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import time
import tomllib
import unittest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
from scripts import board_image  # noqa: E402
from scripts.make_usb_image import (LOG_README, LOG_SECTORS, SECTOR, add_log_partition, add_store_partition,  # noqa: E402
                                    check_image, log_volume, pack_volume)

VVFAT_START = 63  # where make_usb_image.py's vvfat puts the boot partition in a local build
RELEASE_START = 2048  # where pack_volume puts it in a release's image
WHEN = 1763668800  # 2025-11-20 20:00 UTC
PAYLOADS = {"README.TXT": LOG_README}  # the stand-in boot volume is a log volume: its README is its one file
BOARDS = [name for name, _ in board_image.names()]


def image(path, start=VVFAT_START):
    """An image laid out as make_usb_image.py lays one out: a FAT16 boot partition at `start`, then the log and store."""
    mbr = bytearray(SECTOR)
    struct.pack_into("<B3sB3sII", mbr, 446, 0x80, b"\xfe\xff\xff", 0xEF, b"\xfe\xff\xff", start, LOG_SECTORS)
    mbr[510:] = b"\x55\xaa"
    with path.open("wb") as disk:
        disk.write(mbr)
        disk.seek(start * SECTOR)
        disk.write(log_volume(start, time.localtime()))
    add_log_partition(path, time.localtime())
    add_store_partition(path)
    check_image(path, PAYLOADS)
    return path


def volume(path):
    """A boot volume alone, as a release's: a FAT16 volume told it starts at 1 MiB."""
    path.write_bytes(log_volume(RELEASE_START, time.gmtime(WHEN)))
    return path


def firmware(path, e, size=600 * 1024):
    """A stand-in for the entry's firmware: its header at its offset, then bytes that are not blank."""
    blob = bytearray((i * 7 + 3) & 0xFF for i in range(size))
    header, at = e["firmware"]["header"].encode(), e["firmware"]["header-offset"]
    blob[at:at + len(header)] = header
    path.write_bytes(bytes(blob))
    return path


def entries(path):
    with path.open("rb") as disk:
        return board_image.partitions(disk.read(SECTOR))


def sha256(path):
    import hashlib
    return hashlib.sha256(path.read_bytes()).hexdigest()


class Cards(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory(prefix="board-image-")
        self.addCleanup(self.dir.cleanup)
        self.root = Path(self.dir.name)

    def check_card(self, card, source, e, blob):
        before, after = entries(source), entries(card)
        self.assertEqual([k for k, _, _ in after], [0xEF, 0x0E, 0x7F])
        self.assertEqual([n for _, _, n in after], [n for _, _, n in before])
        boot, log, store = (start for _, start, _ in after)
        self.assertEqual(boot, e["card"]["boot-start"])
        self.assertEqual(log % 2048, 0)
        self.assertEqual(store % 2048, 0)
        # Each volume reads back whole where it moved to; the FATs name their new start.
        check_image(card, PAYLOADS)
        with source.open("rb") as old, card.open("rb") as new:
            for (_, was, sectors), (_, now, _) in zip(before, after):
                old.seek(was * SECTOR)
                new.seek(now * SECTOR)
                first_old, first_new = bytearray(old.read(SECTOR)), bytearray(new.read(SECTOR))
                if first_old[510:] == b"\x55\xaa" and first_old[54:59] == b"FAT16":
                    self.assertEqual(struct.unpack_from("<I", first_new, 28)[0], now)
                    struct.pack_into("<I", first_old, 28, now)
                self.assertEqual(first_new, first_old)
                self.assertEqual(new.read((sectors - 1) * SECTOR), old.read((sectors - 1) * SECTOR))
            old.seek(0)
            new.seek(0)
            self.assertEqual(new.read(446), old.read(446))  # the boot code and the disk's signature
            new.seek(e["card"]["firmware-offset"])
            self.assertEqual(new.read(len(blob)), blob)
        # Nothing between the MBR and the firmware, nor between the firmware and the boot partition.
        with card.open("rb") as new:
            new.seek(SECTOR)
            self.assertFalse(any(new.read(e["card"]["firmware-offset"] - SECTOR)))
            new.seek(e["card"]["firmware-offset"] + len(blob))
            self.assertFalse(any(new.read(e["card"]["boot-start"] * SECTOR - e["card"]["firmware-offset"] - len(blob))))

    def test_each_entrys_card_from_a_local_build(self):
        self.assertEqual(BOARDS, ["orangepi-zero-plus2", "orangepi-zero3"])
        source = image(self.root / "mind.img")
        for name in BOARDS:
            with self.subTest(board=name):
                e = board_image.entry(name)
                fw = firmware(self.root / f"{name}.bin", e)
                card = self.root / f"{name}.img"
                board_image.make(source, fw, card, e)
                self.check_card(card, source, e, fw.read_bytes())

    def test_each_entrys_card_from_a_releases_image(self):
        source = self.root / "release.img"
        pack_volume(volume(self.root / "boot.fat"), source, {}, WHEN)
        self.assertEqual(entries(source)[0][1], RELEASE_START)
        for name in BOARDS:
            with self.subTest(board=name):
                e = board_image.entry(name)
                fw = firmware(self.root / f"{name}.bin", e)
                card = self.root / f"{name}.img"
                board_image.make(source, fw, card, e)
                self.check_card(card, source, e, fw.read_bytes())

    def test_a_boot_volume_alone_gives_the_same_card_each_time(self):
        boot = volume(self.root / "boot.fat")
        e = board_image.entry("orangepi-zero3")
        fw = firmware(self.root / "fw.bin", e)
        one, two = self.root / "one.img", self.root / "two.img"
        board_image.from_volume(boot, fw, one, e, WHEN)
        board_image.from_volume(boot, fw, two, e, WHEN)
        self.assertEqual(sha256(one), sha256(two))
        # It is the card of the release's image around the same volume, and the volume is unchanged.
        release = self.root / "release.img"
        pack_volume(boot, release, {}, WHEN)
        three = self.root / "three.img"
        board_image.make(release, fw, three, e)
        self.assertEqual(sha256(one), sha256(three))
        self.check_card(one, release, e, fw.read_bytes())
        self.assertEqual(boot.read_bytes(), log_volume(RELEASE_START, time.gmtime(WHEN)))
        self.assertEqual(sorted(p.name for p in self.root.iterdir()), ["boot.fat", "fw.bin", "one.img", "release.img", "three.img", "two.img"])

    def test_refusals(self):
        source = image(self.root / "mind.img")
        e = board_image.entry("orangepi-zero-plus2")
        card = self.root / "card.img"
        plain = self.root / "plain.bin"
        plain.write_bytes(bytes(4096))
        with self.assertRaisesRegex(ValueError, "eGON.BT0"):
            board_image.make(source, plain, card, e)
        big = firmware(self.root / "big.bin", e, size=e["card"]["boot-start"] * SECTOR)
        with self.assertRaisesRegex(ValueError, "past the boot partition"):
            board_image.make(source, big, card, e)
        small = firmware(self.root / "fw.bin", e)
        board_image.make(source, small, card, e)
        with self.assertRaisesRegex(ValueError, "already exists"):
            board_image.make(source, small, card, e)
        board_image.make(source, small, card, e, force=True)
        with self.assertRaisesRegex(ValueError, "already exists"):
            board_image.from_volume(volume(self.root / "boot.fat"), small, card, e, WHEN)
        two = self.root / "two.img"
        with source.open("rb") as disk:
            data = bytearray(disk.read(SECTOR))
        data[478:494] = bytes(16)  # no store partition
        two.write_bytes(bytes(data))
        with self.assertRaisesRegex(ValueError, "boot, log and store"):
            board_image.make(two, small, self.root / "out.img", e)
        self.assertFalse((self.root / "out.img").exists())
        odd = self.root / "odd.fat"
        odd.write_bytes(log_volume(63, time.gmtime(WHEN)))  # a volume told it starts elsewhere than at 1 MiB
        with self.assertRaisesRegex(ValueError, "Invalid boot volume"):
            board_image.from_volume(odd, small, self.root / "out.img", e, WHEN)
        self.assertFalse((self.root / "out.img").exists())

    def test_the_command(self):
        source = image(self.root / "mind.img")
        e = board_image.entry("orangepi-zero3")
        fw = firmware(self.root / "fw.bin", e)
        card = self.root / "card.img"
        tool = [sys.executable, str(ROOT / "scripts/board_image.py"), "--board", "orangepi-zero3"]
        done = subprocess.run([*tool, "--image", str(source), "--firmware", str(fw), "--output", str(card)], capture_output=True, text=True)
        self.assertEqual(done.returncode, 0, done.stderr)
        self.assertIn(f"SHA-256 {sha256(card)}", done.stdout)
        self.assertIn("not the catalog's", done.stdout)  # a stand-in is not the firmware the catalog names
        env = {k: v for k, v in os.environ.items() if k != "SOURCE_DATE_EPOCH"}
        done = subprocess.run([*tool, "--volume", str(volume(self.root / "boot.fat")), "--firmware", str(fw), "--output", str(self.root / "v.img")],
                              capture_output=True, text=True, env=env)
        self.assertEqual(done.returncode, 2, done.stderr)
        self.assertIn("--volume needs --date", done.stderr)
        done = subprocess.run([*tool, "--volume", str(self.root / "boot.fat"), "--firmware", str(fw), "--output", str(self.root / "v.img")],
                              capture_output=True, text=True, env={**env, "SOURCE_DATE_EPOCH": str(WHEN)})
        self.assertEqual(done.returncode, 0, done.stderr)
        # The entry's script gives the firmware when none is named (board_uboot.sh takes MIND_BOARD_UBOOT as it).
        done = subprocess.run([*tool, "--image", str(source), "--output", str(self.root / "s.img")], capture_output=True, text=True,
                              env={**os.environ, "MIND_BOARD_UBOOT": str(fw)})
        self.assertEqual(done.returncode, 0, done.stderr)
        self.assertEqual(sha256(self.root / "s.img"), sha256(card))
        done = subprocess.run([sys.executable, str(ROOT / "scripts/board_image.py"), "--board", "pinephone", "--image", str(source)],
                              capture_output=True, text=True)
        self.assertEqual(done.returncode, 1)
        self.assertIn("no device 'pinephone' in boards.toml; it has orangepi-zero-plus2, orangepi-zero3", done.stderr)


class Catalog(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory(prefix="board-catalog-")
        self.addCleanup(self.dir.cleanup)
        self.text = board_image.CATALOG.read_text()

    def variant(self, old, new):
        self.assertIn(old, self.text)
        path = Path(self.dir.name) / "boards.toml"
        path.write_text(self.text.replace(old, new, 1))
        return path

    def test_every_entry(self):
        for name in BOARDS:
            with self.subTest(board=name):
                e = board_image.entry(name)
                self.assertEqual(e["arch"], "aarch64")
                self.assertEqual(e["card"]["firmware"], "card")
                self.assertTrue((ROOT / e["card"]["test-tree"]).is_file(), e["card"]["test-tree"])
                self.assertTrue(os.access(ROOT / e["firmware"]["script"], os.X_OK), e["firmware"]["script"])
                self.assertEqual(e["firmware"]["sources"], ["u-boot", "tf-a"])
                self.assertEqual(e["firmware"]["file"], "u-boot-sunxi-with-spl.bin")
                self.assertEqual((e["card"]["firmware-offset"], e["card"]["boot-start"]), (8192, 8192))
        self.assertTrue(board_image.entry("orangepi-zero-plus2")["emmc"]["present"])
        self.assertFalse(board_image.entry("orangepi-zero3")["emmc"]["present"])

    def test_refused_entries(self):
        with self.assertRaisesRegex(ValueError, "only firmware on the card is made yet"):
            board_image.entry("orangepi-zero3", self.variant('firmware = "card"\nfirmware-offset = 8192\nboot-start = 8192\ndevice-tree = "allwinner',
                                                             'firmware = "flash"\nfirmware-offset = 8192\nboot-start = 8192\ndevice-tree = "allwinner'))
        with self.assertRaisesRegex(ValueError, r"\[orangepi-zero-plus2\] lacks soc"):
            board_image.entry("orangepi-zero-plus2", self.variant('soc = "Allwinner H5', 'chip = "Allwinner H5'))
        with self.assertRaisesRegex(ValueError, "sha256 is not 64"):
            board_image.entry("orangepi-zero3", self.variant('sha256 = "73bcdf3f', 'sha256 = "73BCDF3F'))
        with self.assertRaisesRegex(ValueError, "before the boot partition"):
            board_image.entry("orangepi-zero-plus2", self.variant("boot-start = 8192  # sectors", "boot-start = 8  # sectors"))
        with self.assertRaisesRegex(ValueError, "sources tf-b are not in"):
            board_image.entry("orangepi-zero3", self.variant('sources = ["u-boot", "tf-a"]\nu-boot-defconfig = "orangepi_zero3',
                                                             'sources = ["u-boot", "tf-b"]\nu-boot-defconfig = "orangepi_zero3'))

    def test_third_party_names_each_source_as_pinned(self):
        with board_image.CATALOG.open("rb") as f:
            sources = tomllib.load(f)["source"]
        record = (ROOT / "THIRD_PARTY.md").read_text()
        section = record[record.index("## The Allwinner boards' firmware"):]
        section = section[:section.index("\n## ", 1)]
        for key, source in sources.items():
            with self.subTest(source=key):
                self.assertIn(f"{source['tag']}, commit `{source['commit']}`", section)
                self.assertIn(source["name"], section)
        self.assertIn("scripts/boards.toml", section)

    def test_board_uboot_reads_the_entry(self):
        script = ROOT / "scripts/board_uboot.sh"
        done = subprocess.run(["bash", str(script), "nonesuch"], capture_output=True, text=True)
        self.assertEqual(done.returncode, 2)
        self.assertIn("the boards: orangepi-zero-plus2 orangepi-zero3", done.stderr)
        stand_in = Path(self.dir.name) / "u-boot.bin"
        stand_in.write_bytes(b"x")
        done = subprocess.run(["bash", str(script), "orangepi-zero3"], capture_output=True, text=True, env={**os.environ, "MIND_BOARD_UBOOT": str(stand_in)})
        self.assertEqual((done.returncode, done.stdout.strip()), (0, str(stand_in)), done.stderr)
        for name in BOARDS:
            e = board_image.entry(name)
            shown = subprocess.run(["bash", "-c", 'eval "$(python3 scripts/board_image.py --board "$1" --shell)" && '
                                    'echo "$PLAT|$DEFCONFIG|$FILE|$KNOWN|$UBOOT_TAG|$UBOOT_COMMIT|$TFA_TAG|$TFA_COMMIT|${UBOOT_URLS[1]}|${TFA_URLS[0]}"', "-", name],
                                   cwd=ROOT, capture_output=True, text=True)
            u, t = e["source"]["u-boot"], e["source"]["tf-a"]
            self.assertEqual(shown.stdout.strip().split("|"), [e["firmware"]["tf-a-platform"], e["firmware"]["u-boot-defconfig"], e["firmware"]["file"],
                                                              e["firmware"]["sha256"], u["tag"], u["commit"], t["tag"], t["commit"], u["urls"][1], t["urls"][0]])


if __name__ == "__main__":
    unittest.main()
