#!/usr/bin/env python3
"""Build MIND CORE and package a raw UEFI USB image (x86_64, or aarch64 with --arch aarch64); never access physical disks."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess
import re
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
import boot_slots  # noqa: E402
import release as release_tool  # noqa: E402
import sign_manifest  # noqa: E402
# Services (BOOT_FILES in the ABI) are needed by the bootloader, those the signed manifest lists; apps are all other
# *.elf built by 02_build.sh.
BOOT_FILES = re.findall(r'"([\w-]+\.elf)"', re.search(r"BOOT_FILES[^=]*=\s*\[(.*?)\];", (ROOT / "common/abi.rs").read_text(), re.S)[1])
# What differs between the architectures: where 02_build.sh puts the files, the bootloader's name, the ELF machine
# (EM_X86_64, EM_AARCH64), the services built for it (aarch64 has no PS/2, IDE or AC97 driver), the image's name.
ARCHES = {
    "x86_64": {"root": "usb_root", "efi": "EFI/BOOT/BOOTX64.EFI", "machine": b">\x00", "missing": (), "image": "dist/mind-core-usb.img",
               "boot": "UEFI x64, Secure Boot disabled. This is not a Legacy BIOS image."},
    "aarch64": {"root": "aarch64_root", "efi": "EFI/BOOT/BOOTAA64.EFI", "machine": b"\xb7\x00", "missing": ("ps2_kbd.elf", "ata.elf", "audio_gw.elf"),
                "image": "dist/mind-core-usb-aarch64.img",
                "boot": "UEFI AArch64, Secure Boot disabled: QEMU virt with AAVMF (./03_run_qemu_aarch64.sh --image), boards with UEFI firmware (issue 205)."},
}
# Licences travel with the image: tts.elf, hear.elf and voice.elf embed third-party dictionaries, the text programs the MIND Mono
# font (THIRD_PARTY.md).
LICENSES = ("LICENSES/LICENSE-MIT", "LICENSES/LICENSE-APACHE", "LICENSES/THIRD_PARTY.md", *sorted(f"LICENSES/{p.name}" for p in (ROOT / "LICENSES").glob("*.txt")))
# The boot manifest and its signature: the bootloader refuses a volume without them (350-UPD-0002).
SIGNED = ("MANIFEST", "MANIFEST.SIG")
# The voice recognizer's model and grammar (hear and voice, issues 078-079).
VOICE = ("voice/model.bin", "voice/commands.txt")
SECTOR = 512
# The log partition after the boot one (211-PRT-0006): FAT16 with an ordinary MBR type (0x0E, FAT16 LBA), which Windows,
# macOS and Linux mount and write. vfs_server mounts it as log: by its label and saves each boot's system log there.
LOG_LABEL = b"MIND LOG   "
LOG_SECTORS = 64 * 1024 * 1024 // SECTOR
LOG_ALIGN = 2048  # 1 MiB
LOG_SPC = 4  # 2 KiB clusters: about 32 700 of them
LOG_README = (b"MIND CORE writes the system log of each boot here, as BOOTNNNN.LOG (the last 50 boots are kept),\r\n"
              b"the same boot's hardware report as HWNNNN.TXT, and the firmware's ACPI tables in the folder ACPI.\r\n"
              b"This partition is an ordinary FAT16 volume: read it on any computer, and send the logs with a report\r\n"
              b"of what happened (docs/write-disk.md, section 9). On MIND CORE it is log: (ls log:, cat log:boot0001.log).\r\n")


def listed(source):
    """The files the signed manifest in `source` lists, or None without one. A boot service it does not list may be
    absent, as the bootloader allows: one of the other architecture, or one a build left out."""
    manifest = Path(source) / "MANIFEST"
    if not manifest.is_file():
        return None
    return {line.split()[1] for line in manifest.read_text(errors="replace").splitlines() if line.startswith("file ") and len(line.split()) > 1}


def applications(source):
    """The applications in `source`: every *.elf but the kernel and the boot services, listed when the image is made, after
    the build (175-PRT-0007: a list taken at import missed every program of a clean tree's first build)."""
    return tuple(sorted(p.name for p in Path(source).glob("*.elf") if p.name != "kernel.elf" and p.name not in BOOT_FILES))


def files(arch, source=None):
    """The files of the image for `arch`: the bootloader, the kernel, its boot services, the applications, licences, voice."""
    spec = ARCHES[arch]
    source = Path(source or ROOT / spec["root"])
    names = listed(source)
    boot = tuple(name for name in BOOT_FILES if name not in spec["missing"] and (names is None or name in names))
    return (spec["efi"], "kernel.elf", *boot, *applications(source), *LICENSES, *VOICE, *SIGNED)


def find_qemu_img(requested):
    candidates = [requested] if requested else [
        "qemu-img", "qemu-img.exe",
        "/mnt/c/msys64/ucrt64/bin/qemu-img.exe",
        "/mnt/c/msys64/mingw64/bin/qemu-img.exe",
        "/mnt/c/Program Files/qemu/qemu-img.exe",
    ]
    for name in candidates:
        found = shutil.which(name)
        if found:
            return found
    raise ValueError("qemu-img not found. Install QEMU or pass --qemu-img /path/to/qemu-img.")


def qemu_path(path, executable):
    """Windows QEMU under WSL/MSYS needs a Windows path, including drive letters."""
    path = str(path.resolve())
    if executable.lower().endswith(".exe") and os.name != "nt":
        converter = shutil.which("wslpath") or shutil.which("cygpath")
        if not converter:
            raise ValueError("Windows QEMU requires wslpath/cygpath; otherwise use a native qemu-img.")
        path = subprocess.check_output([converter, "-w", path], text=True).strip()
    return path


def read_payloads(source, arch="x86_64"):
    payloads = {}
    machine = ARCHES[arch]["machine"]
    for name in files(arch, source):
        file = source / name
        if not file.is_file():
            raise ValueError(f"Missing {file}. Run the build without --no-build.")
        # The current bootloader's ELF read buffer is 4 MiB.
        if not 0 < file.stat().st_size <= 4 * 1024 * 1024:
            raise ValueError(f"Invalid size of {file}: expected 1..4194304 bytes.")
        data = file.read_bytes()
        if name in LICENSES or name in VOICE or name in SIGNED:
            pass
        elif name.endswith(".elf"):
            if len(data) < 64 or data[:6] != b"\x7fELF\x02\x01" or data[18:20] != machine:
                raise ValueError(f"{file} is not an ELF64 {arch} little-endian binary.")
        elif data[:2] != b"MZ":
            raise ValueError(f"{file} is not a PE/EFI application.")
        payloads[name] = data
    return payloads


def slots(payloads, arch, channel=None):
    """The payloads laid out in slots (351-UPD-0016, docs/update/slots.md), as `boot_slots.py layout` lays out a volume:
    the kernel, the boot services and the signed manifest in MIND/A, confirmed by MIND/BOOT0, with MIND/BOOT1 empty;
    the applications, licences and voice stay at the root, shared. With `channel`, a release's signed channel file
    kept in MIND/A/CHANNEL, so the updater knows the version: it must verify with the release key and name this
    manifest for `arch`."""
    boot = {"kernel.elf", *BOOT_FILES, *SIGNED}
    out = {(f"MIND/A/{name}" if name in boot else name): data for name, data in payloads.items()}
    out["MIND/BOOT0"] = boot_slots.record(1, "A", confirmed=True)
    out["MIND/BOOT1"] = bytes(boot_slots.RECORD)
    if channel is not None:
        lines = channel.split(b"\n")
        if len(lines) != 3 or lines[2] or not lines[1].startswith(b"ed25519 "):
            raise ValueError("the channel file is not a signed channel")
        body = lines[0] + b"\n"
        try:
            signature = bytes.fromhex(lines[1][8:].decode())
        except ValueError:
            raise ValueError("the channel file is not a signed channel") from None
        # The key the updater is built with (updater/build.rs).
        key = Path(os.environ.get("MIND_RELEASE_PUBLIC_KEY") or ROOT / "updater/keys/release-test.pub")
        if not sign_manifest.verify(bytes.fromhex(key.read_text().strip()), body, signature):
            raise ValueError(f"the channel file is not signed with the release key the updater is built with ({key})")
        try:
            name = json.loads(body)["channel"]
        except (ValueError, TypeError, KeyError):
            name = ""
        fields, why = release_tool.channel_from(body, name)
        if fields is None:
            raise ValueError(f"the channel file: {why}")
        if fields["manifests"].get(arch) != hashlib.sha256(payloads["MANIFEST"]).hexdigest():
            raise ValueError(f"the channel does not name this build's {arch} manifest")
        out["MIND/A/CHANNEL"] = channel
    return out


def fat_time(when):
    """FAT date and time of a struct_time (2-second resolution)."""
    return ((when.tm_year - 1980) << 9 | when.tm_mon << 5 | when.tm_mday,
            when.tm_hour << 11 | when.tm_min << 5 | when.tm_sec // 2)


def log_volume(start, when):
    """The log partition's FAT16 volume: boot sector, two FATs, the root with the label and README.TXT."""
    reserved, fats, root_entries = 1, 2, 512
    root_sectors = root_entries * 32 // SECTOR
    fat_sectors = 1
    for _ in range(3):
        clusters = (LOG_SECTORS - reserved - fats * fat_sectors - root_sectors) // LOG_SPC
        fat_sectors = ((clusters + 2) * 2 + SECTOR - 1) // SECTOR
    clusters = (LOG_SECTORS - reserved - fats * fat_sectors - root_sectors) // LOG_SPC
    assert 4085 <= clusters < 65525
    volume = bytearray(LOG_SECTORS * SECTOR)
    boot = bytearray(SECTOR)
    boot[0:3] = b"\xeb\x3c\x90"
    boot[3:11] = b"MINDCORE"
    struct.pack_into("<HBHBHHBHHHII", boot, 11, SECTOR, LOG_SPC, reserved, fats, root_entries, 0, 0xF8, fat_sectors, 63, 255,
                     start, LOG_SECTORS)
    boot[36], boot[38] = 0x80, 0x29
    struct.pack_into("<I", boot, 39, 0x4D4C4F47)  # the volume ID
    boot[43:54] = LOG_LABEL
    boot[54:62] = b"FAT16   "
    boot[510:512] = b"\x55\xaa"
    volume[:SECTOR] = boot
    date, time_ = fat_time(when)
    fat = bytearray(fat_sectors * SECTOR)
    struct.pack_into("<HHH", fat, 0, 0xFFF8, 0xFFFF, 0xFFFF)  # media, clean, README.TXT in cluster 2 alone
    for copy in range(fats):
        at = (reserved + copy * fat_sectors) * SECTOR
        volume[at:at + len(fat)] = fat
    root = (reserved + fats * fat_sectors) * SECTOR
    label = bytearray(32)
    label[0:11], label[11] = LOG_LABEL, 0x08
    struct.pack_into("<HH", label, 22, time_, date)
    readme = bytearray(32)
    readme[0:11], readme[11] = b"README  TXT", 0x20
    struct.pack_into("<HHH", readme, 14, time_, date, date)  # created, accessed
    struct.pack_into("<HHHI", readme, 22, time_, date, 2, len(LOG_README))  # written, first cluster, size
    volume[root:root + 64] = label + readme
    data = root + root_sectors * SECTOR
    volume[data:data + len(LOG_README)] = LOG_README
    return bytes(volume)


def add_log_partition(image, when):
    """Appends the log partition at the next MiB after the image and enters it in the MBR's second slot."""
    size = image.stat().st_size
    start = -(-size // SECTOR // LOG_ALIGN) * LOG_ALIGN
    with image.open("r+b") as disk:
        mbr = bytearray(disk.read(SECTOR))
        if size % SECTOR or any(mbr[462:510]):
            raise ValueError("Invalid USB image: not whole sectors, or more than one partition before the log partition")
        mbr[462:478] = struct.pack("<B3sB3sII", 0, b"\xfe\xff\xff", 0x0E, b"\xfe\xff\xff", start, LOG_SECTORS)
        disk.seek(0)
        disk.write(mbr)
        disk.seek(start * SECTOR)
        disk.write(log_volume(start, when))
        disk.flush()
        os.fsync(disk.fileno())


def check_log_partition(disk, mbr, image_size, boot_end):
    """The second MBR entry is the log partition: FAT16 LBA after the boot one, labelled MIND LOG."""
    def require(condition, message):
        if not condition:
            raise ValueError(f"Invalid USB image: {message}")
    require(not any(mbr[478:510]), "more than two partitions")
    require(mbr[466] == 0x0E, "log partition type (FAT16 LBA)")
    start, length = struct.unpack_from("<II", mbr, 470)
    require(start >= boot_end and start % LOG_ALIGN == 0 and length == LOG_SECTORS and (start + length) * SECTOR <= image_size,
            "log partition placement")
    disk.seek(start * SECTOR)
    boot = disk.read(SECTOR)
    bps, spc, reserved, fats, roots = struct.unpack_from("<HBHBH", boot, 11)
    require(boot[510:] == b"\x55\xaa" and bps == SECTOR and spc == LOG_SPC and fats == 2 and roots == 512
            and struct.unpack_from("<I", boot, 32)[0] == LOG_SECTORS and struct.unpack_from("<I", boot, 28)[0] == start,
            "log partition FAT16 parameters")
    require(boot[38] == 0x29 and boot[43:54] == LOG_LABEL and boot[54:62] == b"FAT16   ", "log partition label")


def check_image(image, payloads, mark_esp=False):
    """Check MBR/FAT16 geometry and all packaged file contents independently of QEMU.

    Only reads regular image files; mark_esp changes the generated partition's
    type to the UEFI-defined MBR ESP type (0xEF), preserving its FAT16 geometry.
    """
    def require(condition, message):
        if not condition:
            raise ValueError(f"Invalid USB image: {message}")

    with image.open("r+b" if mark_esp else "rb") as disk:
        mbr = disk.read(SECTOR)
        require(len(mbr) == SECTOR and mbr[510:] == b"\x55\xaa", "MBR signature")
        require(not any(mbr[462:478]) or mbr[466] == 0x0E, "expected the boot partition and at most the log partition")
        require(mbr[450] in (0x04, 0x06, 0x0e, 0xef), "FAT16/ESP partition type")
        start, length = struct.unpack_from("<II", mbr, 454)
        require(start > 0 and length > 0 and (start + length) * SECTOR <= image.stat().st_size,
                "partition extends beyond the image")
        disk.seek(start * SECTOR)
        boot = disk.read(SECTOR)
        require(len(boot) == SECTOR and boot[510:] == b"\x55\xaa", "FAT boot sector")
        bps, spc, reserved, fats, roots, total16 = struct.unpack_from("<HBHBHH", boot, 11)
        fat_sectors = struct.unpack_from("<H", boot, 22)[0]
        total = total16 or struct.unpack_from("<I", boot, 32)[0]
        require(bps == SECTOR and spc > 0 and spc & (spc - 1) == 0 and reserved > 0
                and fats == 2 and roots > 0 and fat_sectors > 0 and total == length,
                "FAT16 parameters")
        root_sectors = (roots * 32 + SECTOR - 1) // SECTOR
        first_data = reserved + fats * fat_sectors + root_sectors
        clusters = (total - first_data) // spc
        require(4085 <= clusters < 65525 and (clusters + 2) * 2 <= fat_sectors * SECTOR,
                "FAT16 cluster count")
        disk.seek((start + reserved) * SECTOR)
        fat = disk.read(fat_sectors * SECTOR)
        require(fat == disk.read(fat_sectors * SECTOR), "FAT copies differ")
        root = disk.read(root_sectors * SECTOR)
        cluster_size = spc * SECTOR

        def chain(first):
            result, visited = bytearray(), set()
            while first < 0xfff8:
                require(2 <= first < clusters + 2 and first not in visited, "cluster chain")
                visited.add(first)
                require(len(visited) * cluster_size <= 4 * 1024 * 1024 + cluster_size,
                        "cluster chain too long")
                disk.seek((start + first_data + (first - 2) * spc) * SECTOR)
                result.extend(disk.read(cluster_size))
                first = struct.unpack_from("<H", fat, first * 2)[0]
            return bytes(result)

        def lookup(directory, part):
            stem, _, extension = part.upper().partition(".")
            short_name = (stem.ljust(8) + extension.ljust(3)).encode("ascii")
            long_parts = {}  # names longer than 8.3 (compositor.elf, vfs_server.elf) are stored in LFN entries
            for offset in range(0, len(directory), 32):
                entry = directory[offset:offset + 32]
                if entry[0] == 0:
                    break
                if entry[0] == 0xe5:
                    long_parts = {}
                    continue
                if entry[11] == 0x0f:
                    chars = entry[1:11] + entry[14:26] + entry[28:32]
                    long_parts[entry[0] & 0x1f] = chars.decode("utf-16-le").split("\0")[0].rstrip("\uffff")
                    continue
                long_name = "".join(long_parts[i] for i in sorted(long_parts))
                long_parts = {}
                if not entry[11] & 8 and (entry[:11] == short_name or long_name.lower() == part.lower()):
                    return entry
            raise ValueError(f"USB image is missing {part}")

        for name, expected in payloads.items():
            directory = root
            parts = name.split("/")
            for part in parts[:-1]:
                entry = lookup(directory, part)
                require(entry[11] & 0x10, f"{part} is not a directory")
                directory = chain(struct.unpack_from("<H", entry, 26)[0])
            entry = lookup(directory, parts[-1])
            size = struct.unpack_from("<I", entry, 28)[0]
            require(not entry[11] & 0x10 and size == len(expected), f"size of {name}")
            actual = chain(struct.unpack_from("<H", entry, 26)[0])[:size]
            require(actual == expected, f"contents of {name} do not match the build output")
        if any(mbr[462:478]):
            check_log_partition(disk, mbr, image.stat().st_size, start + length)
        if mark_esp:
            disk.seek(450)
            disk.write(b"\xef")
            disk.flush()
            os.fsync(disk.fileno())


def pack(payloads, output, qemu_img, arch, force=False):
    """The payloads packed into a raw image at `output` (MBR, FAT16 marked as the ESP, the log partition), checked
    against them before it replaces anything; returns the image's SHA-256."""
    output.parent.mkdir(parents=True, exist_ok=True)
    # Stage ONLY boot files. Never include old test disks, image files or local
    # firmware variables from usb_root. Conversion never touches the live tree.
    with tempfile.TemporaryDirectory(prefix=".mind-usb-", dir=output.parent) as work:
        work = Path(work)
        source = work / "files"
        for name, data in payloads.items():
            target = source / name
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(data)
        # The writable directory: files written there (`write data/…`) can be read on any computer afterwards.
        (source / "data").mkdir(exist_ok=True)
        temporary = work / "disk.img"
        descriptor = {"driver": "raw", "file": {
            "driver": "vvfat", "dir": qemu_path(source, qemu_img),
            "fat-type": 16, "floppy": False, "rw": False, "label": "MIND CORE",
        }}
        print(f">>> Creating RAW USB image (MBR, FAT16, UEFI {arch})...", flush=True)
        subprocess.run([qemu_img, "convert", "-O", "raw", "json:" + json.dumps(descriptor),
                        qemu_path(temporary, qemu_img)], check=True)
        check_image(temporary, payloads, mark_esp=True)
        add_log_partition(temporary, time.localtime())
        check_image(temporary, payloads)
        digest = hashlib.sha256()
        with temporary.open("rb") as disk:
            for chunk in iter(lambda: disk.read(1024 * 1024), b""):
                digest.update(chunk)
        # Failed conversion/verification leaves any prior image untouched.
        if force:
            os.replace(temporary, output)
        else:
            # Atomic no-clobber publication also detects a competing build.
            os.link(temporary, output)
            temporary.unlink()
    return digest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--arch", choices=sorted(ARCHES), default="x86_64", help="the architecture of the image (default: x86_64)")
    parser.add_argument("--output", type=Path,
                        help="path to the .img (default: dist/mind-core-usb.img, dist/mind-core-usb-aarch64.img for aarch64)")
    parser.add_argument("--no-build", action="store_true", help="use the already built usb_root/ (aarch64_root/ for aarch64)")
    parser.add_argument("--force", action="store_true", help="overwrite an existing image file")
    parser.add_argument("--qemu-img", default=os.environ.get("QEMU_IMG"), help="path to qemu-img[.exe]")
    parser.add_argument("--hwdocs", action="store_true", help="also put the hardware tables of hwdocs/ in /hwdocs (gpio and pins read them)")
    parser.add_argument("--slots", action="store_true", help="the build in slot A, confirmed, so the updater can fill slot B (docs/update/slots.md)")
    parser.add_argument("--channel", type=Path, help="with --slots: the release's signed channel file, kept in slot A (its manifest must be this build's)")
    parser.add_argument("--update", type=Path, help="with --slots: an update.txt for the updater, at the root (docs/update/updater.md)")
    args = parser.parse_args()
    if (args.channel or args.update) and not args.slots:
        raise ValueError("--channel and --update go with --slots.")
    spec = ARCHES[args.arch]
    output = (args.output or ROOT / spec["image"]).absolute()
    if output.suffix.lower() != ".img":
        raise ValueError("Output file must have the .img extension.")
    if output.is_symlink() or (output.exists() and not output.is_file()):
        raise ValueError("Output path must be a regular file, not a symlink or device.")
    if output.exists() and not args.force:
        raise ValueError(f"{output} already exists. Pass --force to overwrite it.")
    qemu_img = find_qemu_img(args.qemu_img)
    if not args.no_build:
        print(f">>> Building the project for {args.arch}...", flush=True)
        subprocess.run(["bash", str(ROOT / "02_build.sh")], cwd=ROOT, check=True, env={**os.environ, "ARCH": args.arch})
    payloads = read_payloads(ROOT / spec["root"], args.arch)
    if args.hwdocs:
        # Not part of the system: only on request, so the image stays the same without them (hwdocs/README.md).
        for file in sorted((ROOT / "hwdocs").rglob("*")):
            if file.is_file() and file.name != "README.md":
                payloads[file.relative_to(ROOT).as_posix()] = file.read_bytes()
    if args.slots:
        payloads = slots(payloads, args.arch, args.channel.read_bytes() if args.channel else None)
        if args.update:
            payloads["update.txt"] = args.update.read_bytes()
    digest = pack(payloads, output, qemu_img, args.arch, args.force)
    print(f">>> Done: {output}\nSize: {output.stat().st_size} bytes\n"
          f"SHA256: {digest.hexdigest()}\n"
          "Write the .img to the whole USB drive in RAW/DD mode"
          f"{'' if args.arch == 'x86_64' else ' (./05_write_usb_linux.sh --image ' + str(output.relative_to(ROOT) if output.is_relative_to(ROOT) else output) + ')'}.\n"
          f"Boot: {spec['boot']}")


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f"Error: {error}", file=sys.stderr)
        sys.exit(1)
