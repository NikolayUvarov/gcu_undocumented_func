#!/usr/bin/env python3
"""A device's card from a MIND Core aarch64 image or boot volume and the device's firmware, as its catalog entry says
(100004-MOB-0001, 352-MOB-0010).

The catalog, scripts/boards.toml, says for each device which firmware it takes, where the firmware goes and where the boot
partition starts. An Allwinner board's boot ROM reads U-Boot's SPL from 8 KiB into the card, and the SPL, BL31 and U-Boot
run on from there for up to about 1 MiB. make_usb_image.py's boot partition starts inside that: at sector 63 in a local
build, at 1 MiB in a release's image. So the card is the image with its three partitions moved past the firmware, each at
its MiB boundary as before, their FAT volumes told their new start, and the firmware at its offset. A boot volume alone (a
release's FAT image) is first laid out as make_usb_image.py lays out a release's image. The image or volume is never
changed; only regular files are written.
"""
import argparse
import hashlib
import os
from pathlib import Path
import shlex
import struct
import subprocess
import sys
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
from scripts.make_usb_image import SECTOR, LOG_ALIGN, STORE_ALIGN, check_image, pack_volume  # noqa: E402

CATALOG = ROOT / "scripts/boards.toml"
CHUNK = 1024 * 1024
# What an entry must say, by table ("" is the entry's own).
FIELDS = {"": ("title", "soc", "arch", "main-task"),
          "firmware": ("script", "sources", "file", "header", "header-offset", "sha256", "built-with"),
          "card": ("firmware", "firmware-offset", "boot-start", "device-tree", "test-tree"),
          "emmc": ("present", "note")}
# What board_uboot.sh takes from an entry whose firmware is built from U-Boot and TF-A.
UBOOT_FIELDS = ("u-boot-defconfig", "tf-a-platform")


def entry(name, catalog=CATALOG):
    """The catalog's entry `name`, checked, with its sources' records under "source"; ValueError names what is wrong."""
    with open(catalog, "rb") as f:
        data = tomllib.load(f)
    boards, sources = data.get("board", {}), data.get("source", {})
    if name not in boards:
        raise ValueError(f"no device {name!r} in {Path(catalog).name}; it has {', '.join(sorted(boards)) or 'none'}")
    e = boards[name]
    for table, fields in FIELDS.items():
        part = e if not table else e.get(table)
        if not isinstance(part, dict):
            raise ValueError(f"{name}: no [{table}] table")
        missing = [f for f in fields if f not in part]
        if missing:
            raise ValueError(f"{name}: [{table or name}] lacks {', '.join(missing)}")
    firmware, card, emmc = e["firmware"], e["card"], e["emmc"]
    if e["arch"] != "aarch64":
        raise ValueError(f"{name}: arch {e['arch']!r}: only aarch64 images are made into cards")
    if card["firmware"] != "card":
        raise ValueError(f"{name}: firmware {card['firmware']!r}: only firmware on the card is made yet; on the device's own "
                         "flash it is the PinePhone's, which waits for the phone (100000)")
    unknown = [s for s in firmware["sources"] if s not in sources]
    if unknown:
        raise ValueError(f"{name}: sources {', '.join(unknown)} are not in [source]")
    if {"u-boot", "tf-a"} <= set(firmware["sources"]) and any(f not in firmware for f in UBOOT_FIELDS):
        raise ValueError(f"{name}: [firmware] lacks {', '.join(f for f in UBOOT_FIELDS if f not in firmware)}")
    digest = firmware["sha256"]
    if not (isinstance(digest, str) and len(digest) == 64 and all(c in "0123456789abcdef" for c in digest)):
        raise ValueError(f"{name}: sha256 is not 64 lower-case hex digits")
    places = [("card", card)] + ([("emmc", emmc)] if emmc["present"] else [])
    for table, place in places:
        offset, start = place.get("firmware-offset"), place.get("boot-start")
        if not (isinstance(offset, int) and isinstance(start, int) and offset >= SECTOR and offset % SECTOR == 0
                and start % LOG_ALIGN == 0 and offset < start * SECTOR):
            raise ValueError(f"{name}: [{table}] firmware-offset and boot-start: the firmware after the MBR, whole sectors, "
                             "before the boot partition, which starts at a MiB")
    header, at = firmware["header"], firmware["header-offset"]
    if not (isinstance(header, str) and header.isascii() and header and isinstance(at, int) and at >= 0):
        raise ValueError(f"{name}: header and header-offset")
    return {**e, "name": name, "source": {s: sources[s] for s in firmware["sources"]}}


def names(catalog=CATALOG):
    """The catalog's devices, in order, with their titles."""
    with open(catalog, "rb") as f:
        return [(name, e.get("title", "")) for name, e in tomllib.load(f).get("board", {}).items()]


def shell(e):
    """The firmware's build as shell assignments, for scripts/board_uboot.sh."""
    if not {"u-boot", "tf-a"} <= set(e["source"]):
        raise ValueError(f"{e['name']}: its firmware is not built from U-Boot and TF-A")
    firmware, uboot, tfa = e["firmware"], e["source"]["u-boot"], e["source"]["tf-a"]
    values = {"PLAT": firmware["tf-a-platform"], "DEFCONFIG": firmware["u-boot-defconfig"], "FILE": firmware["file"], "KNOWN": firmware["sha256"],
              "KNOWN_WITH": firmware["built-with"], "UBOOT_TAG": uboot["tag"], "UBOOT_COMMIT": uboot["commit"],
              "TFA_TAG": tfa["tag"], "TFA_COMMIT": tfa["commit"]}
    lines = [f"{k}={shlex.quote(str(v))}" for k, v in values.items()]
    lines += [f"UBOOT_URLS=({' '.join(map(shlex.quote, uboot['urls']))})", f"TFA_URLS=({' '.join(map(shlex.quote, tfa['urls']))})"]
    return "\n".join(lines)


def partitions(mbr):
    """The MBR's entries as (type, start, sectors), in slot order, for the slots in use."""
    if len(mbr) != SECTOR or mbr[510:] != b"\x55\xaa":
        raise ValueError("not an MBR: no signature")
    entries = []
    for slot in range(4):
        part = mbr[446 + 16 * slot:462 + 16 * slot]
        kind, (start, sectors) = part[4], struct.unpack_from("<II", part, 8)
        if kind:
            entries.append((kind, start, sectors))
    return entries


def check_firmware(blob, e):
    """Refuses a file without the entry's header, or one that would reach the boot partition; returns its end in bytes."""
    firmware, card = e["firmware"], e["card"]
    header, at = firmware["header"].encode(), firmware["header-offset"]
    if blob[at:at + len(header)] != header:
        raise ValueError(f"firmware: no {firmware['header']} header at byte {at}: not {firmware['file']}")
    end = card["firmware-offset"] + len(blob)
    if end > card["boot-start"] * SECTOR:
        raise ValueError(f"firmware ends at byte {end}, past the boot partition at {card['boot-start'] * SECTOR}")
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


def make(image, firmware, output, e, force=False):
    """Writes the card of device `e` (an entry) from `image` to `output` and returns its SHA-256."""
    blob = Path(firmware).read_bytes()
    firmware_end = check_firmware(blob, e)
    offset, boot_start = e["card"]["firmware-offset"], e["card"]["boot-start"]
    with Path(image).open("rb") as source:
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
            target.seek(offset)
            target.write(blob)
            target.flush()
            os.fsync(target.fileno())
    check_image(temporary, {})
    with temporary.open("rb") as card:
        if any(start * SECTOR < firmware_end for _, start, _ in partitions(card.read(SECTOR))):
            raise ValueError("a partition overlaps the firmware")
        card.seek(offset)
        if card.read(len(blob)) != blob:
            raise ValueError("the firmware did not read back as written")
        digest = hashlib.sha256()
        card.seek(0)
        for chunk in iter(lambda: card.read(CHUNK), b""):
            digest.update(chunk)
    os.replace(temporary, output)
    return digest.hexdigest()


def from_volume(volume, firmware, output, e, when, force=False):
    """The card of device `e` from a boot volume alone (a FAT image at 1 MiB, as release.py makes one), its log volume
    dated `when` (seconds, UTC), so one volume and one firmware give the same card; returns its SHA-256."""
    if os.path.exists(output) and not force:
        raise ValueError(f"{output} already exists. Pass --force to overwrite it.")
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".mind-board-", dir=output.parent) as work:
        image = Path(work) / "image.img"
        pack_volume(volume, image, {}, when, force=True)
        return make(image, firmware, output, e, force)


def firmware_of(e):
    """The firmware's file, from the entry's script, which prints its path last."""
    done = subprocess.run([str(ROOT / e["firmware"]["script"]), e["name"]], cwd=ROOT, stdout=subprocess.PIPE, text=True)
    lines = [line for line in done.stdout.splitlines() if line.strip()]
    if done.returncode or not lines or not Path(lines[-1]).is_file():
        raise ValueError(f"{e['firmware']['script']} {e['name']}: no firmware (exit {done.returncode})")
    return Path(lines[-1])


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--list", action="store_true", help="the catalog's devices")
    parser.add_argument("--board", help="the device, by its name in scripts/boards.toml")
    parser.add_argument("--shell", action="store_true", help="the device's firmware build as shell assignments (board_uboot.sh)")
    given = parser.add_mutually_exclusive_group()
    given.add_argument("--image", type=Path, help="make_usb_image.py's aarch64 image, or a release's mind-core-<version>-aarch64.img")
    given.add_argument("--volume", type=Path, help="a boot volume alone: a FAT image at 1 MiB, as a release's is")
    parser.add_argument("--date", type=int, help="with --volume: the log volume's date in seconds (the release's), else $SOURCE_DATE_EPOCH")
    parser.add_argument("--firmware", type=Path, help="the device's firmware file (default: the one its entry's script builds)")
    parser.add_argument("--output", type=Path, help="the card's image (default: dist/boards/mind-<board>.img)")
    parser.add_argument("--force", action="store_true", help="overwrite an existing output file")
    args = parser.parse_args()
    try:
        if args.list:
            for name, title in names():
                print(f"{name}\t{title}")
            return
        if not args.board:
            parser.error("--board <name> (or --list)")
        e = entry(args.board)
        if args.shell:
            print(shell(e))
            return
        if not (args.image or args.volume):
            parser.error("--image <file> or --volume <file>")
        when = args.date if args.date is not None else int(os.environ["SOURCE_DATE_EPOCH"]) if os.environ.get("SOURCE_DATE_EPOCH") else None
        if args.volume and when is None:
            parser.error("--volume needs --date <seconds> or $SOURCE_DATE_EPOCH, so that one volume gives the same card")
        firmware = args.firmware or firmware_of(e)
        output = (args.output or ROOT / f"dist/boards/mind-{e['name']}.img").absolute()
        output.parent.mkdir(parents=True, exist_ok=True)
        if args.volume:
            digest = from_volume(args.volume, firmware, output, e, when, args.force)
        else:
            digest = make(args.image, firmware, output, e, args.force)
        got = hashlib.sha256(firmware.read_bytes()).hexdigest()
        known = e["firmware"]["sha256"]
        said = "the catalog's" if got == known else f"not the catalog's {known[:16]}… from {e['firmware']['built-with']}"
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        sys.exit(f"board_image: {error}")
    print(f"{e['title']}: the firmware {firmware}, SHA-256 {got} ({said})\n{output}\nSHA-256 {digest}")


if __name__ == "__main__":
    main()
