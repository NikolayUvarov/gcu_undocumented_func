#!/usr/bin/env python3
"""An Allwinner board's card from a MIND Core aarch64 image and the board's U-Boot (100004-MOB-0001).

The board's boot ROM reads U-Boot's SPL from 8 KiB into the card, and `u-boot-sunxi-with-spl.bin` (the SPL, BL31 and
U-Boot) runs on from there for up to about 1 MiB. make_usb_image.py's boot partition starts at sector 63, inside it.
So the card is the image with its three partitions moved past U-Boot, each at its MiB boundary as before, their FAT
volumes told their new start, and U-Boot at 8 KiB. The image itself is never changed; only regular files are written.
"""
import argparse
import hashlib
import os
from pathlib import Path
import struct
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
from scripts.make_usb_image import SECTOR, LOG_ALIGN, STORE_ALIGN, check_image  # noqa: E402

SPL_OFFSET = 8 * 1024  # where the A64's, the H5's and the H616's boot ROMs read the SPL on a card
BOOT_START = 4 * 1024 * 1024 // SECTOR  # room for a U-Boot of almost 4 MiB
MAGIC = b"eGON.BT0"  # the SPL's header, at byte 4 (U-Boot's include/sunxi_image.h)
CHUNK = 1024 * 1024


def partitions(mbr):
    """The MBR's entries as (type, start, sectors), in slot order, for the slots in use."""
    if len(mbr) != SECTOR or mbr[510:] != b"\x55\xaa":
        raise ValueError("not an MBR: no signature")
    entries = []
    for slot in range(4):
        entry = mbr[446 + 16 * slot:462 + 16 * slot]
        kind, (start, sectors) = entry[4], struct.unpack_from("<II", entry, 8)
        if kind:
            entries.append((kind, start, sectors))
    return entries


def check_uboot(uboot, boot_start):
    """Refuses a file that is not a sunxi SPL image, or that would reach the first partition."""
    if len(uboot) < 32 or uboot[4:12] != MAGIC:
        raise ValueError(f"U-Boot: no {MAGIC.decode()} header at byte 4: not u-boot-sunxi-with-spl.bin")
    end = SPL_OFFSET + len(uboot)
    if end > boot_start * SECTOR:
        raise ValueError(f"U-Boot ends at byte {end}, past the first partition at {boot_start * SECTOR}")
    return end


def layout(entries, boot_start):
    """The new starts: the boot partition at boot_start, the log and the store each at the next MiB after the one before."""
    starts, end = [], boot_start
    for index, (_, _, sectors) in enumerate(entries):
        align = 1 if index == 0 else (LOG_ALIGN if index == 1 else STORE_ALIGN)
        start = -(-end // align) * align
        starts.append(start)
        end = start + sectors
    return starts, end


def copy(source, target, at, to, sectors):
    """Copies a partition's sectors, leaving blank chunks unwritten (the store is blank after its superblock)."""
    source.seek(at * SECTOR)
    left, offset = sectors * SECTOR, to * SECTOR
    while left:
        data = source.read(min(CHUNK, left))
        if not data:
            raise ValueError("the image ends inside a partition")
        if any(data):
            target.seek(offset)
            target.write(data)
        offset += len(data)
        left -= len(data)


def hidden(target, start):
    """A FAT volume's boot sector says how many sectors come before it (BPB offset 28): its new start."""
    target.seek(start * SECTOR)
    boot = bytearray(target.read(SECTOR))
    if boot[510:] != b"\x55\xaa":
        raise ValueError(f"no FAT boot sector at sector {start}")
    struct.pack_into("<I", boot, 28, start)
    target.seek(start * SECTOR)
    target.write(boot)


def make(image, uboot, output, boot_start=BOOT_START, force=False):
    """Writes the card's image to `output` and returns its SHA-256."""
    blob = uboot.read_bytes()
    uboot_end = check_uboot(blob, boot_start)
    with image.open("rb") as source:
        mbr = bytearray(source.read(SECTOR))
        entries = partitions(mbr)
        kinds = [kind for kind, _, _ in entries]
        if kinds != [0xEF, 0x0E, 0x7F]:
            raise ValueError(f"expected make_usb_image.py's boot, log and store partitions, found types {[hex(k) for k in kinds]}")
        starts, end = layout(entries, boot_start)
        if os.path.exists(output) and not force:
            raise ValueError(f"{output} already exists. Pass --force to overwrite it.")
        temporary = output.with_name(output.name + ".part")
        with temporary.open("w+b") as target:
            target.truncate(end * SECTOR)
            for (kind, at, sectors), to in zip(entries, starts):
                copy(source, target, at, to, sectors)
            for slot, ((kind, _, sectors), to) in enumerate(zip(entries, starts)):
                struct.pack_into("<II", mbr, 446 + 16 * slot + 8, to, sectors)
            target.seek(0)
            target.write(mbr)
            hidden(target, starts[0])
            hidden(target, starts[1])
            target.seek(SPL_OFFSET)
            target.write(blob)
            target.flush()
            os.fsync(target.fileno())
    check_image(temporary, {})
    with temporary.open("rb") as card:
        if any(start * SECTOR < uboot_end for _, start, _ in partitions(card.read(SECTOR))):
            raise ValueError("a partition overlaps U-Boot")
        card.seek(SPL_OFFSET)
        if card.read(len(blob)) != blob:
            raise ValueError("U-Boot did not read back as written")
        digest = hashlib.sha256()
        card.seek(0)
        for chunk in iter(lambda: card.read(CHUNK), b""):
            digest.update(chunk)
    os.replace(temporary, output)
    return digest.hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--image", type=Path, required=True, help="make_usb_image.py's aarch64 image")
    parser.add_argument("--uboot", type=Path, required=True, help="the board's u-boot-sunxi-with-spl.bin (scripts/board_uboot.sh)")
    parser.add_argument("--output", type=Path, required=True, help="the card's image")
    parser.add_argument("--boot-start", type=int, default=BOOT_START, help=f"the boot partition's first sector (default {BOOT_START}, 4 MiB)")
    parser.add_argument("--force", action="store_true", help="overwrite an existing output file")
    args = parser.parse_args()
    try:
        digest = make(args.image, args.uboot, args.output.absolute(), args.boot_start, args.force)
    except (OSError, ValueError) as e:
        sys.exit(f"board_image: {e}")
    print(f"{args.output}\nSHA-256 {digest}")


if __name__ == "__main__":
    main()
