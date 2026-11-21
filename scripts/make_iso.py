#!/usr/bin/env python3
"""A hybrid installer ISO of MIND Core for UEFI (352-UPD-0047), x86_64 or aarch64; never touches a physical disk.

One file boots from a disc, a stick or a virtual machine's drive:
- an ISO 9660 volume whose El Torito catalog has one EFI entry, the boot volume: a FAT16 image with the same files as
  make_usb_image.py's, under the build's signed manifest;
- an MBR in the ISO's system area whose one entry, the EFI system partition, is that same image, so firmware that boots
  the file as a disk finds the same volume.
The boot volume is the ISO's last extent: a firmware that reads the catalog's count of 1 as "to the end of the disc"
(as EDK2 does) sees the whole volume, which is larger than the count's 16 bits can say.
Every date in the ISO and the volume is the build's date (--date, SOURCE_DATE_EPOCH or the commit's), so the same
build gives the same bytes. Needs mkfs.fat and mtools.
"""
import argparse
import hashlib
import os
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
from scripts import make_usb_image  # noqa: E402

BLOCK = 2048  # an ISO 9660 logical block
SECTOR = 512
PVD, BOOT_RECORD, TERMINATOR, L_PATH, M_PATH, ROOT_DIR, CATALOG, README = range(16, 24)
VOLUME_LBA = 512  # the boot volume at 1 MiB, so its MBR entry starts at sector 2048, as on a disk
VOLUME_ID = "MIND_CORE"
FAT_LABEL = "MIND CORE"
FAT_SERIAL = "4d494e44"  # "MIND": the volume ID is a constant, not the time of the build
SLACK_MB = 32  # room on the boot volume beyond its files, and a second slot's room when it is in slots
MIN_MB = 64
NAMES = {"catalog": "BOOT.CAT;1", "volume": "MINDBOOT.IMG;1", "readme": "README.TXT;1"}
README_TEXT = (b"MIND Core installer (352-UPD-0047)\r\n\r\n"
               b"Boot this disc or stick on a UEFI machine (Secure Boot off unless its owner enrolled MIND Core's keys).\r\n"
               b"MINDBOOT.IMG is the boot volume: the same files as the USB image, under the same signed manifest.\r\n")
MTOOLS_ENV = {"MTOOLS_SKIP_CHECK": "1"}


def both16(value):
    return struct.pack("<H", value) + struct.pack(">H", value)


def both32(value):
    return struct.pack("<I", value) + struct.pack(">I", value)


def text(value, size, fill=b" "):
    data = value.encode("ascii")
    if len(data) > size:
        raise ValueError(f"{value!r} is longer than {size} bytes")
    return data + fill * (size - len(data))


def record_date(when):
    """A directory record's 7-byte date: years since 1900, month, day, hour, minute, second, offset from UTC."""
    t = time.gmtime(when)
    return bytes([t.tm_year - 1900, t.tm_mon, t.tm_mday, t.tm_hour, t.tm_min, t.tm_sec, 0])


def volume_date(when):
    """A volume descriptor's 17-byte date: YYYYMMDDHHMMSS, hundredths, offset from UTC."""
    return time.strftime("%Y%m%d%H%M%S", time.gmtime(when)).encode() + b"00\x00"


def directory_record(name, lba, size, when, directory=False):
    name = name if isinstance(name, bytes) else name.encode("ascii")
    length = 33 + len(name) + (len(name) + 1) % 2
    return (bytes([length, 0]) + both32(lba) + both32(size) + record_date(when) + bytes([2 if directory else 0, 0, 0])
            + both16(1) + bytes([len(name)]) + name + b"\x00" * ((len(name) + 1) % 2))


def blocks(size):
    return -(-size // BLOCK)


def files_to_stage(payloads, work, when):
    """The payloads as files under `work`, with data/ and data/machine/ as make_usb_image.py makes them, all dated `when`."""
    source = work / "files"
    for name, data in payloads.items():
        target = source / name
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
    (source / "data" / "machine").mkdir(parents=True, exist_ok=True)
    for path in sorted(source.rglob("*"), reverse=True):
        os.utime(path, (when, when))
    return source


def boot_volume(payloads, path, when, start=VOLUME_LBA * BLOCK // SECTOR):
    """A FAT16 image at `path` holding the payloads, told it starts at `start` (in 512-byte sectors); returns its size."""
    content = sum(-(-len(data) // 4096) * 4096 for data in payloads.values())
    # In slots, room for slot B as large as slot A, so that a disk the installer copies this volume to can be updated.
    slot = sum(-(-len(data) // 4096) * 4096 for name, data in payloads.items() if name.startswith("MIND/A/"))
    megabytes = max(MIN_MB, -(-(content + slot) // (1 << 20)) + SLACK_MB)
    env = dict(os.environ, **MTOOLS_ENV, SOURCE_DATE_EPOCH=str(int(when)))
    with tempfile.TemporaryDirectory(prefix=".mind-iso-", dir=path.parent) as work:
        source = files_to_stage(payloads, Path(work), when)
        with path.open("wb") as image:
            image.truncate(megabytes << 20)
        subprocess.run(["mkfs.fat", "--invariant", "-F", "16", "-i", FAT_SERIAL, "-n", FAT_LABEL, "-h", str(start),
                        str(path), str(megabytes << 10)], check=True, capture_output=True, env=env)
        entries = sorted(str(p) for p in source.iterdir())
        subprocess.run(["mcopy", "-s", "-m", "-i", str(path), *entries, "::"], check=True, capture_output=True, env=env)
    return path.stat().st_size


def catalog(volume_size, platform):
    """The El Torito boot catalog: a validation entry for `platform` (0xEF: EFI) and one bootable no-emulation entry."""
    validation = bytearray(32)
    validation[0], validation[1] = 1, platform
    validation[4:28] = text("MIND CORE", 24, b"\x00")
    validation[30:32] = b"\x55\xaa"
    struct.pack_into("<H", validation, 28, -sum(struct.unpack("<16H", validation)) & 0xFFFF)
    count = volume_size // SECTOR
    entry = bytearray(32)
    entry[0] = 0x88  # bootable, no emulation
    struct.pack_into("<HI", entry, 6, count if count <= 0xFFFF else 1, VOLUME_LBA)
    return bytes(validation + entry).ljust(BLOCK, b"\x00")


def mbr(volume_size, serial):
    """The ISO's first 512 bytes: an MBR whose one entry, the EFI system partition, is the boot volume."""
    sector = bytearray(SECTOR)
    struct.pack_into("<I", sector, 440, serial)
    struct.pack_into("<B3sB3sII", sector, 446, 0x80, b"\xfe\xff\xff", 0xEF, b"\xfe\xff\xff",
                     VOLUME_LBA * BLOCK // SECTOR, volume_size // SECTOR)
    sector[510:] = b"\x55\xaa"
    return bytes(sector)


def descriptors(total, when, application):
    root = directory_record(b"\x00", ROOT_DIR, BLOCK, when, directory=True)
    pvd = bytearray(BLOCK)
    pvd[0:8] = b"\x01CD001\x01\x00"
    pvd[8:40] = text("", 32)
    pvd[40:72] = text(VOLUME_ID, 32)
    pvd[80:88] = both32(total)
    pvd[120:124], pvd[124:128], pvd[128:132] = both16(1), both16(1), both16(BLOCK)
    pvd[132:140] = both32(10)
    struct.pack_into("<I", pvd, 140, L_PATH)
    struct.pack_into(">I", pvd, 148, M_PATH)
    pvd[156:190] = root
    pvd[190:318] = text("", 128)
    pvd[318:446] = text("MIND CORE", 128)
    pvd[446:574] = text("", 128)
    pvd[574:702] = text(application, 128)
    pvd[702:813] = text("", 111)
    date = volume_date(when)
    pvd[813:830], pvd[830:847] = date, date
    pvd[847:864] = b"0" * 16 + b"\x00"
    pvd[864:881] = date
    pvd[881] = 1
    boot = bytearray(BLOCK)
    boot[0:7] = b"\x00CD001\x01"
    boot[7:39] = text("EL TORITO SPECIFICATION", 32, b"\x00")
    struct.pack_into("<I", boot, 71, CATALOG)
    end = bytearray(BLOCK)
    end[0:7] = b"\xffCD001\x01"
    path_l = bytearray(BLOCK)
    path_l[0:10] = bytes([1, 0]) + struct.pack("<IH", ROOT_DIR, 1) + b"\x00\x00"
    path_m = bytearray(BLOCK)
    path_m[0:10] = bytes([1, 0]) + struct.pack(">IH", ROOT_DIR, 1) + b"\x00\x00"
    return bytes(pvd + boot + end + path_l + path_m)


def root_directory(volume_size, when):
    rows = [directory_record(b"\x00", ROOT_DIR, BLOCK, when, True), directory_record(b"\x01", ROOT_DIR, BLOCK, when, True),
            directory_record(NAMES["catalog"], CATALOG, BLOCK, when),
            directory_record(NAMES["volume"], VOLUME_LBA, volume_size, when),
            directory_record(NAMES["readme"], README, len(README_TEXT), when)]
    return b"".join(rows).ljust(BLOCK, b"\x00")


def write_iso(volume, output, when, arch):
    """The ISO at `output` around the boot volume image `volume`; returns its SHA-256."""
    size = volume.stat().st_size
    if size % BLOCK:
        raise ValueError(f"the boot volume's size {size} is not a multiple of {BLOCK}")
    total = VOLUME_LBA + size // BLOCK
    serial = int(FAT_SERIAL, 16) ^ int(when)
    platform = 0xEF
    digest = hashlib.sha256()
    with output.open("wb") as iso:
        head = bytearray(VOLUME_LBA * BLOCK)
        head[0:SECTOR] = mbr(size, serial)
        head[PVD * BLOCK:ROOT_DIR * BLOCK] = descriptors(total, when, f"MIND CORE {arch} (352-UPD-0047)")
        head[ROOT_DIR * BLOCK:CATALOG * BLOCK] = root_directory(size, when)
        head[CATALOG * BLOCK:README * BLOCK] = catalog(size, platform)
        head[README * BLOCK:README * BLOCK + len(README_TEXT)] = README_TEXT
        if len(head) != VOLUME_LBA * BLOCK:
            raise ValueError("the ISO's first MiB is not laid out whole")
        iso.write(head)
        digest.update(head)
        with volume.open("rb") as source:
            for chunk in iter(lambda: source.read(1 << 20), b""):
                iso.write(chunk)
                digest.update(chunk)
        iso.flush()
        os.fsync(iso.fileno())
    check_iso(output)
    return digest.hexdigest()


def check_iso(path):
    """Reads the ISO back: its descriptors, catalog, root directory and MBR must all name one boot volume, a FAT16 image
    that knows its start, at the end of the file. Returns (lba, size) of that volume."""
    def require(ok, what):
        if not ok:
            raise ValueError(f"{path}: {what}")
    with path.open("rb") as iso:
        head = iso.read(VOLUME_LBA * BLOCK)
        iso.seek(0, os.SEEK_END)
        length = iso.tell()
        require(len(head) == VOLUME_LBA * BLOCK and length % BLOCK == 0, "too short, or not whole blocks")
        block = lambda n: head[n * BLOCK:(n + 1) * BLOCK]  # noqa: E731
        pvd, boot, end = block(PVD), block(BOOT_RECORD), block(TERMINATOR)
        require(pvd[0:7] == b"\x01CD001\x01" and pvd[881] == 1, "no primary volume descriptor at block 16")
        require(struct.unpack_from("<I", pvd, 80)[0] * BLOCK == length, "the volume's size is not the file's")
        require(struct.unpack_from("<H", pvd, 128)[0] == BLOCK, "the block size is not 2048")
        require(boot[0:7] == b"\x00CD001\x01" and boot[7:30] == b"EL TORITO SPECIFICATION", "no El Torito boot record at block 17")
        require(end[0:7] == b"\xffCD001\x01", "no terminator at block 18")
        require(struct.unpack_from("<I", boot, 71)[0] == CATALOG, "the boot record does not name the catalog")
        cat = block(CATALOG)
        require(cat[0] == 1 and cat[30:32] == b"\x55\xaa" and sum(struct.unpack("<16H", cat[:32])) & 0xFFFF == 0,
                "the catalog's validation entry is bad")
        require(cat[1] == 0xEF, "the catalog's platform is not EFI")
        count, lba = struct.unpack_from("<HI", cat, 38)
        require(cat[32] == 0x88 and cat[33] == 0, "the default entry is not bootable without emulation")
        size = length - lba * BLOCK
        require(lba == VOLUME_LBA and size > 0, "the boot entry is not the volume at 1 MiB")
        require(count == (size // SECTOR if size // SECTOR <= 0xFFFF else 1), "the boot entry's count does not fit the volume")
        rows, at, directory = {}, 0, block(ROOT_DIR)
        while at < BLOCK and directory[at]:
            n = directory[at + 32]
            rows[directory[at + 33:at + 33 + n]] = struct.unpack_from("<II", directory, at + 2)[0:1] + struct.unpack_from("<I", directory, at + 10)
            at += directory[at]
        require(rows.get(NAMES["volume"].encode()) == (lba, size), "the root directory does not list the volume where it is")
        require(rows.get(NAMES["catalog"].encode(), (0,))[0] == CATALOG, "the root directory does not list the catalog")
        part = head[446:462]
        start, sectors = struct.unpack_from("<II", part, 8)
        require(head[510:512] == b"\x55\xaa" and part[4] == 0xEF, "the MBR has no EFI system partition")
        require(start * SECTOR == lba * BLOCK and sectors * SECTOR == size, "the MBR's partition is not the boot volume")
        require(not any(head[462:510]), "the MBR has more than one entry")
        iso.seek(lba * BLOCK)
        fat = iso.read(SECTOR)
        require(fat[510:512] == b"\x55\xaa" and fat[54:62] == b"FAT16   ", "the boot volume is not FAT16")
        require(struct.unpack_from("<I", fat, 28)[0] == start, "the boot volume does not know its start")
        require(struct.unpack_from("<H", fat, 11)[0] == SECTOR, "the boot volume's sectors are not 512 bytes")
        total = struct.unpack_from("<H", fat, 19)[0] or struct.unpack_from("<I", fat, 32)[0]
        require(0 < total * SECTOR <= size, "the boot volume is larger than its extent")
    return lba, size


def build_date():
    if os.environ.get("SOURCE_DATE_EPOCH"):
        return int(os.environ["SOURCE_DATE_EPOCH"])
    out = subprocess.run(["git", "log", "-1", "--format=%ct"], cwd=ROOT, capture_output=True, text=True)
    if out.returncode or not out.stdout.strip():
        raise ValueError("no build date: pass --date or SOURCE_DATE_EPOCH, or run in the git tree")
    return int(out.stdout.strip())


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--arch", choices=sorted(make_usb_image.ARCHES), default="x86_64")
    parser.add_argument("--output", type=Path, help="the .iso (default: dist/mind-core-<arch>.iso)")
    parser.add_argument("--slots", action="store_true", help="the build in slot A, confirmed, as make_usb_image.py --slots")
    parser.add_argument("--channel", type=Path, help="with --slots: the release's signed channel file (make_usb_image.py)")
    parser.add_argument("--keys", type=Path, help="with --channel: the key file it was signed under")
    parser.add_argument("--date", type=int, help="the build's date, seconds since 1970 (default: SOURCE_DATE_EPOCH, else the commit's)")
    parser.add_argument("--force", action="store_true", help="overwrite an existing .iso")
    args = parser.parse_args()
    try:
        if not all(shutil.which(t) for t in ("mkfs.fat", "mcopy")):
            raise ValueError("needs mkfs.fat (dosfstools) and mcopy (mtools)")
        if (args.channel or args.keys) and not args.slots:
            raise ValueError("--channel and --keys go with --slots")
        spec = make_usb_image.ARCHES[args.arch]
        output = (args.output or ROOT / f"dist/mind-core-{args.arch}.iso").absolute()
        if output.suffix.lower() != ".iso":
            raise ValueError("the output must have the .iso extension")
        if output.exists() and not args.force:
            raise ValueError(f"{output} already exists. Pass --force to overwrite it.")
        when = args.date if args.date is not None else build_date()
        payloads = make_usb_image.read_payloads(ROOT / spec["root"], args.arch)
        if args.slots:
            payloads = make_usb_image.slots(payloads, args.arch, args.channel.read_bytes() if args.channel else None,
                                            args.keys.read_bytes() if args.keys else None)
        output.parent.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(prefix=".mind-iso-", dir=output.parent) as work:
            volume = Path(work) / "boot.img"
            boot_volume(payloads, volume, when)
            part = output.with_name(output.name + ".part")
            digest = write_iso(volume, part, when, args.arch)
            os.replace(part, output)
    except (OSError, ValueError, subprocess.CalledProcessError) as e:
        sys.exit(f"make_iso: {e}")
    print(f"{output}\nSHA256: {digest}\nBoot: {spec['boot'].split(',')[0]}, as a disc or written to a stick (dd).")


if __name__ == "__main__":
    main()
