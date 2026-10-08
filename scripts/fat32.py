"""A FAT32 disk image from a list of files: an MBR with one FAT32 partition, contiguous files, long names; no tools needed.

The image is reproducible (fixed timestamps and volume id, sorted names) and sparse where nothing is written. QEMU's vvfat
stops at about 516 MB, which a disk of models passes.
"""
import math
import os
import struct
from pathlib import Path

SECTOR = 512
CLUSTER = 4096  # 8 sectors: FAT32 needs 65525 clusters at least, 256 MiB with 4 KiB clusters
SPC = CLUSTER // SECTOR
PART_START = 2048  # the partition starts at 1 MiB
RESERVED = 32
FATS = 2
MIN_CLUSTERS = 65525 + 16
DATE = ((2026 - 1980) << 9) | (1 << 5) | 1  # 2026-01-01, the date of every entry
END = 0x0FFFFFFF


def _short_names(names):
    """8.3 names for a directory's entries: a basis and ~N, unique within the directory."""
    taken, out = set(), {}
    for name in names:
        stem, _, ext = name.rpartition(".") if "." in name.lstrip(".") else (name, "", "")
        clean = lambda s: "".join(c for c in s.upper() if c.isascii() and (c.isalnum() or c in "$%'-_@~`!(){}^#&"))
        stem, ext = clean(stem) or "FILE", clean(ext)[:3]
        for n in range(1, 1_000_000):
            tail = f"~{n}"
            short = (stem[: 8 - len(tail)] + tail).ljust(8) + ext.ljust(3)
            if short not in taken:
                break
        taken.add(short)
        out[name] = short.encode("ascii")
    return out


def _lfn_entries(name, short):
    """The long-name entries of `name`, last part first, as the 8.3 entry `short` expects them before it."""
    checksum = 0
    for b in short:
        checksum = (((checksum & 1) << 7) + (checksum >> 1) + b) & 0xFF
    units = list(name.encode("utf-16-le"))
    chars = [units[i] | units[i + 1] << 8 for i in range(0, len(units), 2)]
    if len(chars) % 13:
        chars.append(0)  # the terminator, when the last entry has room for it
    while len(chars) % 13:
        chars.append(0xFFFF)
    parts = [chars[i:i + 13] for i in range(0, len(chars), 13)]
    entries = []
    for i, part in enumerate(parts):
        seq = (i + 1) | (0x40 if i == len(parts) - 1 else 0)
        c = struct.pack("<13H", *part)
        entries.append(struct.pack("<B10sBBB12sH4s", seq, c[0:10], 0x0F, 0, checksum, c[10:22], 0, c[22:26]))
    return entries[::-1]


def _entry(short, attributes, cluster, size):
    return struct.pack("<11sBBBHHHHHHHI", short, attributes, 0, 0, 0, DATE, DATE, cluster >> 16, 0, DATE, cluster & 0xFFFF, size)


class _Dir:
    def __init__(self):
        self.dirs, self.files, self.cluster, self.parent = {}, {}, 0, None

    def entry_count(self):
        names = list(self.dirs) + list(self.files)
        return 2 + sum(1 + math.ceil(len(n.encode("utf-16-le")) // 2 / 13) for n in names) + 1


def build(output, files, label="MIND MODELS", extra=0):
    """Write `files` ({path in the volume: source path}) to the image `output`, with `extra` bytes of free space."""
    root = _Dir()
    for path in sorted(files):
        parts = Path(path).parts
        node = root
        for part in parts[:-1]:
            child = node.dirs.setdefault(part, _Dir())
            child.parent = node
            node = child
        node.files[parts[-1]] = Path(files[path])
    # Clusters: the directories first (root at 2), then each file's run in order.
    order, stack = [], [root]
    while stack:
        d = stack.pop(0)
        order.append(d)
        stack.extend(d.dirs[k] for k in sorted(d.dirs))
    used = 2
    for d in order:
        d.cluster = used
        d.clusters = max(1, math.ceil(d.entry_count() * 32 / CLUSTER))
        used += d.clusters
    runs = []
    for d in order:
        for name in sorted(d.files):
            size = d.files[name].stat().st_size
            if size >= 1 << 32:
                raise ValueError(f"{name}: FAT32 holds files under 4 GiB")
            count = math.ceil(size / CLUSTER)
            runs.append((d, name, used if count else 0, size, count))
            used += count
    clusters = max(MIN_CLUSTERS, used - 2 + math.ceil(extra / CLUSTER))
    fat_sectors = math.ceil((clusters + 2) * 4 / SECTOR)
    volume = RESERVED + FATS * fat_sectors + clusters * SPC
    total = PART_START + volume
    out = Path(output)
    with open(out, "wb") as f:
        f.truncate(total * SECTOR)
        # MBR: one partition of type 0x0C (FAT32, LBA).
        mbr = bytearray(SECTOR)
        mbr[446:462] = struct.pack("<B3sB3sII", 0, b"\xfe\xff\xff", 0x0C, b"\xfe\xff\xff", PART_START, volume)
        mbr[440:444] = struct.pack("<I", 0x4D494E44)
        mbr[510:512] = b"\x55\xaa"
        f.seek(0); f.write(mbr)
        base = PART_START * SECTOR
        boot = bytearray(SECTOR)
        boot[0:3] = b"\xeb\x58\x90"
        boot[3:11] = b"MINDCORE"
        boot[11:36] = struct.pack("<HBHBHHBHHHII", SECTOR, SPC, RESERVED, FATS, 0, 0, 0xF8, 0, 63, 255, PART_START, volume)
        boot[36:90] = struct.pack("<IHHIHH12sBBBI11s8s", fat_sectors, 0, 0, 2, 1, 6, b"", 0x80, 0, 0x29, 0x4D4F444C,
                                  label.upper().encode("ascii")[:11].ljust(11), b"FAT32   ")
        boot[510:512] = b"\x55\xaa"
        free = clusters - (used - 2)
        fsinfo = bytearray(SECTOR)
        struct.pack_into("<I", fsinfo, 0, 0x41615252)
        struct.pack_into("<III", fsinfo, 484, 0x61417272, free, used)
        struct.pack_into("<I", fsinfo, 508, 0xAA550000)
        for at in (0, 6):
            f.seek(base + at * SECTOR); f.write(boot)
            f.seek(base + (at + 1) * SECTOR); f.write(fsinfo)
        # The FAT: every directory and file a contiguous chain.
        fat = bytearray((used) * 4)
        struct.pack_into("<II", fat, 0, 0x0FFFFFF8, END)
        chains = [(d.cluster, d.clusters) for d in order] + [(c, n) for _, _, c, _, n in runs if n]
        for start, count in chains:
            for c in range(start, start + count):
                struct.pack_into("<I", fat, c * 4, c + 1 if c + 1 < start + count else END)
        for i in range(FATS):
            f.seek(base + (RESERVED + i * fat_sectors) * SECTOR); f.write(fat)
        data = base + (RESERVED + FATS * fat_sectors) * SECTOR
        at = lambda cluster: data + (cluster - 2) * CLUSTER
        # Directories.
        file_runs = {(id(d), name): (c, size) for d, name, c, size, _ in runs}
        for d in order:
            names = sorted(d.dirs) + sorted(d.files)
            shorts = _short_names(names)
            raw = bytearray()
            if d is root:
                raw += _entry(label.upper().encode("ascii")[:11].ljust(11), 0x08, 0, 0)
            else:
                parent = d.parent.cluster if d.parent is not root else 0
                raw += _entry(b".          ", 0x10, d.cluster, 0) + _entry(b"..         ", 0x10, parent, 0)
            for name in names:
                if name in d.dirs:
                    cluster, size, attributes = d.dirs[name].cluster, 0, 0x10
                else:
                    (cluster, size), attributes = file_runs[(id(d), name)], 0x20
                raw += b"".join(_lfn_entries(name, shorts[name])) + _entry(shorts[name], attributes, cluster, size)
            if len(raw) > d.clusters * CLUSTER:
                raise ValueError("directory larger than planned")
            f.seek(at(d.cluster)); f.write(raw)
        # File data.
        for d, name, cluster, size, count in runs:
            if not count:
                continue
            f.seek(at(cluster))
            with open(d.files[name], "rb") as src:
                for chunk in iter(lambda: src.read(1 << 20), b""):
                    f.write(chunk)
    return {"bytes": total * SECTOR, "clusters": clusters, "free_bytes": free * CLUSTER}


if __name__ == "__main__":
    import sys
    if len(sys.argv) < 3:
        raise SystemExit("usage: fat32.py <output.img> <directory>")
    src = Path(sys.argv[2])
    listing = {p.relative_to(src).as_posix(): p for p in sorted(src.rglob("*")) if p.is_file()}
    print(build(sys.argv[1], listing))
