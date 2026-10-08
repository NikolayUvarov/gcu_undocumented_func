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

ROOT = Path(__file__).resolve().parents[1]
# Services (BOOT_FILES in the ABI) are needed by the bootloader; apps are all other *.elf built by 02_build.sh.
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
APPLICATIONS = tuple(sorted(p.name for p in (ROOT / "usb_root").glob("*.elf") if p.name != "kernel.elf" and p.name not in BOOT_FILES))
# Licences travel with the image: tts.elf, hear.elf and voice.elf embed third-party dictionaries, the text programs the MIND Mono
# font (THIRD_PARTY.md).
LICENSES = ("LICENSES/LICENSE-MIT", "LICENSES/LICENSE-APACHE", "LICENSES/THIRD_PARTY.md", *sorted(f"LICENSES/{p.name}" for p in (ROOT / "LICENSES").glob("*.txt")))
# The boot manifest and its signature: the bootloader refuses a volume without them (350-UPD-0002).
SIGNED = ("MANIFEST", "MANIFEST.SIG")
# The voice recognizer's model and grammar (hear and voice, issues 078-079).
VOICE = ("voice/model.bin", "voice/commands.txt")
FILES = ("EFI/BOOT/BOOTX64.EFI", "kernel.elf", *BOOT_FILES, *APPLICATIONS, *LICENSES, *VOICE, *SIGNED)
SECTOR = 512


def files(arch):
    """The files of the image for `arch`: the bootloader, the kernel, its boot services, the applications, licences, voice."""
    spec = ARCHES[arch]
    if arch == "x86_64":
        return FILES
    boot = tuple(name for name in BOOT_FILES if name not in spec["missing"])
    apps = tuple(sorted(p.name for p in (ROOT / spec["root"]).glob("*.elf") if p.name != "kernel.elf" and p.name not in BOOT_FILES))
    return (spec["efi"], "kernel.elf", *boot, *apps, *LICENSES, *VOICE, *SIGNED)


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
    for name in files(arch):
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
        require(not any(mbr[462:510]), "expected a single partition")
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
        if mark_esp:
            disk.seek(450)
            disk.write(b"\xef")
            disk.flush()
            os.fsync(disk.fileno())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--arch", choices=sorted(ARCHES), default="x86_64", help="the architecture of the image (default: x86_64)")
    parser.add_argument("--output", type=Path,
                        help="path to the .img (default: dist/mind-core-usb.img, dist/mind-core-usb-aarch64.img for aarch64)")
    parser.add_argument("--no-build", action="store_true", help="use the already built usb_root/ (aarch64_root/ for aarch64)")
    parser.add_argument("--force", action="store_true", help="overwrite an existing image file")
    parser.add_argument("--qemu-img", default=os.environ.get("QEMU_IMG"), help="path to qemu-img[.exe]")
    parser.add_argument("--hwdocs", action="store_true", help="also put the hardware tables of hwdocs/ in /hwdocs (gpio and pins read them)")
    args = parser.parse_args()
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
        print(f">>> Creating RAW USB image (MBR, FAT16, UEFI {args.arch})...", flush=True)
        subprocess.run([qemu_img, "convert", "-O", "raw", "json:" + json.dumps(descriptor),
                        qemu_path(temporary, qemu_img)], check=True)
        check_image(temporary, payloads, mark_esp=True)
        digest = hashlib.sha256()
        with temporary.open("rb") as disk:
            for chunk in iter(lambda: disk.read(1024 * 1024), b""):
                digest.update(chunk)
        # Failed conversion/verification leaves any prior image untouched.
        if args.force:
            os.replace(temporary, output)
        else:
            # Atomic no-clobber publication also detects a competing build.
            os.link(temporary, output)
            temporary.unlink()
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
