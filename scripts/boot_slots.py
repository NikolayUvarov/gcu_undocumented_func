#!/usr/bin/env python3
"""Slots A and B and the boot records (351-UPD-0006, docs/update/slots.md; MC-9.1, 9.3).

A volume with slots keeps the kernel, the boot services and the signed manifest in MIND/A/ and MIND/B/, and two boot
records, MIND/BOOT0 and MIND/BOOT1, of one sector each. The bootloader follows the record with the higher sequence
number, counts a trial's tries down on the other record before the slot runs, and falls back to the other slot.

Usage:
    boot_slots.py layout VOLUME OUT [--both]   OUT: VOLUME (a signed build) with its boot set in slot A, and in B too
                                               with --both; slot A confirmed
    boot_slots.py show VOLUME|IMAGE            the two records of a directory or a raw disk image (MBR, FAT)
    boot_slots.py stage IMAGE SLOT [--tries N] the record that boots SLOT on trial, N tries (default 3), falling back
                                               to the slot now booted
    boot_slots.py confirm IMAGE                the record that confirms the slot of the newer record

The record is the one in bootloader/src/slots.rs: magic "MINDBOOT", format 1, sequence (u64), slot, fallback (or 0),
tries left, flags (bit 0: confirmed), zeros, and a CRC-32 of the first 508 bytes; all little-endian. The manifest in a
slot is the build's own: the bootloader checks the slot's kernel and services against it by name.
"""
import os
import re
import shutil
import struct
import subprocess
import sys
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
RECORD = 512
MAGIC = b"MINDBOOT"
FORMAT = 1
FILES = ("MIND/BOOT0", "MIND/BOOT1")
# The kernel and the services the bootloader loads (BOOT_FILES in the ABI), and the manifest it checks them against.
BOOT_FILES = re.findall(r'"([\w-]+\.elf)"', re.search(r"BOOT_FILES[^=]*=\s*\[(.*?)\];", (ROOT / "common/abi.rs").read_text(), re.S)[1])
SIGNED = ("MANIFEST", "MANIFEST.SIG")
MTOOLS_ENV = dict(os.environ, MTOOLS_SKIP_CHECK="1")


def record(sequence, slot, fallback=None, tries=0, confirmed=False):
    data = struct.pack("<8sIQ4B", MAGIC, FORMAT, sequence, ord(slot), ord(fallback) if fallback else 0, tries, 1 if confirmed else 0)
    data = data.ljust(RECORD - 4, b"\0")
    return data + struct.pack("<I", zlib.crc32(data))


def parse(data):
    """The record as a dict, or None where the bootloader would ignore it."""
    if len(data) != RECORD or data[:8] != MAGIC or struct.unpack_from("<I", data, RECORD - 4)[0] != zlib.crc32(data[:RECORD - 4]):
        return None
    _, version, sequence, slot, fallback, tries, flags = struct.unpack_from("<8sIQ4B", data)
    if version != FORMAT or chr(slot) not in "AB" or (fallback and chr(fallback) not in "AB") or flags & ~1 or any(data[24:RECORD - 4]):
        return None
    return {"sequence": sequence, "slot": chr(slot), "fallback": chr(fallback) if fallback else None, "tries": tries, "confirmed": bool(flags & 1)}


def boot_set(volume):
    """The files of a volume that go into a slot: the kernel, the boot services it has, the manifest."""
    return [name for name in ("kernel.elf", *BOOT_FILES, *SIGNED) if (Path(volume) / name).is_file()]


def layout(volume, out, both=False):
    """OUT: a copy of VOLUME with its boot set moved into slot A (and copied to B), slot A confirmed."""
    volume, out = Path(volume), Path(out)
    shutil.copytree(volume, out)
    for slot in ("A", "B") if both else ("A",):
        (out / "MIND" / slot).mkdir(parents=True)
        for name in boot_set(volume):
            shutil.copyfile(volume / name, out / "MIND" / slot / name)
    for name in boot_set(volume):
        (out / name).unlink()
    (out / FILES[0]).write_bytes(record(1, "A", confirmed=True))
    (out / FILES[1]).write_bytes(bytes(RECORD))
    return out


class Image:
    """A raw disk image with an MBR and one FAT partition: files read and written through mtools."""
    def __init__(self, path):
        self.path = Path(path)
        with self.path.open("rb") as f:
            mbr = f.read(512)
        self.drive = f"{self.path}@@{struct.unpack_from('<I', mbr, 454)[0] * 512}"

    @classmethod
    def create(cls, path, source, megabytes=64):
        """An image of `megabytes` with an EFI system partition holding FAT16 and the files of `source`."""
        path, start = Path(path), 2048
        sectors = (megabytes << 20) // 512 - start
        with path.open("wb") as f:
            f.truncate(megabytes << 20)
            mbr = bytearray(512)
            mbr[446:462] = struct.pack("<B3sB3sII", 0x80, b"\xfe\xff\xff", 0xEF, b"\xfe\xff\xff", start, sectors)
            mbr[510:512] = b"\x55\xaa"
            f.write(mbr)
        subprocess.run(["mkfs.fat", "-F", "16", "-n", "MINDSLOTS", "--offset", str(start), "-h", str(start), str(path), str(sectors // 2)], check=True, capture_output=True)
        image = cls(path)
        subprocess.run(["mcopy", "-s", "-i", image.drive, *[str(p) for p in sorted(Path(source).iterdir())], "::"], check=True, env=MTOOLS_ENV, capture_output=True)
        return image

    def read(self, name):
        result = subprocess.run(["mtype", "-i", self.drive, f"::{name}"], capture_output=True, env=MTOOLS_ENV)
        return result.stdout if result.returncode == 0 else None

    def write(self, name, data):
        temp = self.path.with_name(self.path.name + ".write")
        temp.write_bytes(data)
        subprocess.run(["mcopy", "-o", "-i", self.drive, str(temp), f"::{name}"], check=True, env=MTOOLS_ENV, capture_output=True)
        temp.unlink()


def records(volume):
    """The two records of a directory or an image, each parsed or None."""
    if Path(volume).is_dir():
        read = lambda name: (Path(volume) / name).read_bytes() if (Path(volume) / name).is_file() else None
    else:
        read = Image(volume).read
    return [parse(data) if data is not None else None for data in map(read, FILES)]


def newest(found):
    """(file index, record) of the newer valid record, or None."""
    valid = [(k, r) for k, r in enumerate(found) if r]
    return max(valid, key=lambda kr: (kr[1]["sequence"], -kr[0])) if valid else None


def write_next(image, **fields):
    """Writes a record one past the newer valid one into the other file, as the updater will; returns it."""
    found = newest(records(image.path))
    k, sequence = (found[0], found[1]["sequence"]) if found else (1, 0)
    image.write(FILES[1 - k], record(sequence + 1, **fields))
    return parse(image.read(FILES[1 - k]))


def main(argv):
    if argv[:1] == ["layout"] and len(argv) in (3, 4) and argv[3:] in ([], ["--both"]):
        out = layout(argv[1], argv[2], argv[3:] == ["--both"])
        print(f"SLOTS IN {out}: {', '.join(sorted(p.name for p in (out / 'MIND').iterdir() if p.is_dir()))}; SLOT A CONFIRMED")
    elif argv[:1] == ["show"] and len(argv) == 2:
        found = records(argv[1])
        for name, r in zip(FILES, found):
            print(f"{name}: {r if r else 'NONE OR DAMAGED'}")
        chosen = newest(found)
        print(f"NEWER: {FILES[chosen[0]]}" if chosen else "NO VALID RECORD")
    elif argv[:1] == ["stage"] and len(argv) in (3, 5) and argv[2] in ("A", "B") and argv[3:4] in ([], ["--tries"]):
        image = Image(argv[1])
        current = newest(records(image.path))
        fallback = current[1]["slot"] if current and current[1]["confirmed"] else (current[1]["fallback"] if current else None)
        print(write_next(image, slot=argv[2], fallback=fallback, tries=int(argv[4]) if len(argv) == 5 else 3))
    elif argv[:1] == ["confirm"] and len(argv) == 2:
        image = Image(argv[1])
        current = newest(records(image.path))
        if not current:
            print("NO VALID RECORD")
            return 1
        print(write_next(image, slot=current[1]["slot"], fallback=current[1]["fallback"], confirmed=True))
    else:
        print(__doc__)
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
