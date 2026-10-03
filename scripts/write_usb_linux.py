#!/usr/bin/env python3
"""Write a MIND CORE image to an explicitly selected, non-system Linux USB disk."""
import argparse
import fcntl
import hashlib
import json
import os
from pathlib import Path
import stat
import struct
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
CHUNK = 4 * 1024 * 1024


def devices():
    return json.loads(subprocess.check_output([
        "lsblk", "--json", "--tree", "--bytes", "--paths", "--output",
        "NAME,PATH,TYPE,SIZE,MODEL,SERIAL,TRAN,RO,LOG-SEC,MAJ:MIN,MOUNTPOINTS",
    ], text=True))["blockdevices"]


def descendants(node):
    yield node
    for child in node.get("children", []):
        yield from descendants(child)


def fingerprint(node):
    return tuple(node.get(k) for k in ("path", "maj:min", "size", "model", "serial", "tran", "log-sec"))


def validate_target(tree, path, size, protected):
    candidates = [n for root in tree for n in descendants(root) if n["path"] == path]
    if len(candidates) != 1:
        raise ValueError("Drive not found or its topology is ambiguous.")
    disk = candidates[0]
    if disk["type"] != "disk" or disk.get("tran") != "usb":
        raise ValueError("A whole USB disk is required, e.g. /dev/sdb, not a partition like /dev/sdb1.")
    if disk["ro"] or disk["log-sec"] != 512 or disk["size"] < size:
        raise ValueError("Disk is write-protected, has a non-512-byte sector size, or is smaller than the image.")
    for node in descendants(disk):
        mounts = node.get("mountpoints") or []
        if node["maj:min"] in protected or any(m in ("/", "/boot", "/boot/efi", "/usr", "/var", "/home") for m in mounts):
            raise ValueError("This is a system disk, or it holds the image, the script or the current directory.")
        if node is not disk and node["type"] != "part":
            raise ValueError("Disk is used by LVM/RAID/crypt or another stacked device.")
        if "[SWAP]" in mounts:
            raise ValueError("Disk has active swap. Writing is not allowed.")
    return disk


def protected_devices(image):
    protected = set()
    for path in [Path(p) for p in ("/", "/boot", "/boot/efi", "/usr", "/var", "/home")] + [image, ROOT, Path.cwd()]:
        if path.exists():
            value = subprocess.check_output(
                ["findmnt", "--noheadings", "--output", "MAJ:MIN", "--target", str(path.resolve())],
                text=True).strip()
            if not value:
                raise ValueError(f"Could not determine the filesystem of {path}.")
            protected.update(value.splitlines())
    return protected


def image_info(source):
    source.seek(0, os.SEEK_END)
    size = source.tell()
    source.seek(0)
    mbr = source.read(512)
    if size % 512 or len(mbr) != 512 or mbr[510:] != b"\x55\xaa" or mbr[450] != 0xef:
        raise ValueError("Expected a RAW MIND CORE image with an MBR/UEFI partition.")
    start, sectors = struct.unpack_from("<II", mbr, 454)
    if start == 0 or sectors == 0 or (start + sectors) * 512 > size:
        raise ValueError("Image partition extends beyond the end of the file.")
    source.seek(start * 512)
    boot = source.read(512)
    if len(boot) != 512 or boot[510:] != b"\x55\xaa" or boot[43:54] != b"MIND CORE  " or boot[54:62] != b"FAT16   ":
        raise ValueError("Image has no MIND CORE FAT16 partition.")
    source.seek(0)
    digest = hashlib.sha256()
    for chunk in iter(lambda: source.read(CHUNK), b""):
        digest.update(chunk)
    source.seek(0)
    return size, digest.hexdigest()


def write_all(target, data):
    view = memoryview(data)
    while view:
        count = target.write(view)
        if not count:
            raise OSError("Incomplete write to the drive.")
        view = view[count:]


def copy_and_verify(source, target, size, disk_size, expected, flush, progress=lambda n: None):
    """Stream core, also exercised with ordinary temporary files in tests."""
    if size <= 0 or size % 512 or disk_size < size or disk_size % 512:
        raise ValueError("Invalid image/drive size.")
    source.seek(0)
    target.seek(0)
    copied = 0
    while copied < size:
        data = source.read(min(CHUNK, size - copied))
        if not data:
            raise OSError("Image changed or was truncated during the write.")
        write_all(target, data)
        copied += len(data)
        progress(copied * 100 // size)
    # Remove any old backup GPT at the physical end, without changing image bytes.
    tail = max(size, disk_size - 33 * 512)
    target.seek(tail)
    write_all(target, bytes(disk_size - tail))
    flush()
    target.seek(0)
    digest = hashlib.sha256()
    remaining = size
    while remaining:
        data = target.read(min(CHUNK, remaining))
        if not data:
            raise OSError("Could not read back the whole written image.")
        digest.update(data)
        remaining -= len(data)
    if digest.hexdigest() != expected:
        raise OSError("SHA256 mismatch: verification failed. The USB drive is not ready.")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--list", action="store_true", help="list USB disks without writing")
    parser.add_argument("--device", help="whole disk: /dev/sdX or /dev/disk/by-id/...")
    parser.add_argument("--image", type=Path, default=ROOT / "dist/mind-core-usb.img")
    parser.add_argument("--check", action="store_true", help="validate the selection without unmounting or writing")
    args = parser.parse_args()
    if not sys.platform.startswith("linux"):
        raise ValueError("This script is for Linux. On Windows use the .ps1 script.")
    tree = devices()
    if args.list or not args.device:
        print("USB disks (PATH | SIZE GiB | MODEL | SERIAL):")
        count = 0
        for disk in tree:
            if disk["type"] == "disk" and disk.get("tran") == "usb":
                print(f"{disk['path']} | {disk['size'] / 2**30:.2f} | {disk.get('model')} | {disk.get('serial')}")
                count += 1
        if not count:
            print("None found. For a USB drive attached to Windows, use the Windows script.")
        if not args.list:
            parser.error("pass --device /dev/sdX; the writer never picks a disk automatically")
        return
    device = Path(args.device).resolve(strict=True)
    if not stat.S_ISBLK(device.stat().st_mode):
        raise ValueError("--device must point to a block device, not a file.")
    image = args.image.resolve(strict=True)
    if not image.is_file():
        raise ValueError("--image must point to a regular image file.")
    with image.open("rb") as source:
        size, digest = image_info(source)
        disk = validate_target(tree, str(device), size, protected_devices(image))
        identity = fingerprint(disk)
        print(f"Image: {image}\nSize: {size} bytes\nSHA256: {digest}\n"
              f"USB: {device} | {disk.get('model')} | {disk.get('serial')} | {disk['size']} bytes")
        mounts = {m for n in descendants(disk) for m in (n.get("mountpoints") or []) if m}
        print("Will unmount: " + (", ".join(sorted(mounts)) or "none"))
        if args.check:
            print("Check passed. Nothing was written.")
            return
        if os.geteuid() != 0:
            raise ValueError("Run the script with sudo to write.")
        if not sys.stdin.isatty():
            raise ValueError("Write confirmation must be entered in an interactive terminal.")
        confirmation = f"ERASE {device}"
        print("ALL DATA ON THE SELECTED USB DISK WILL BE LOST.")
        if input(f"Type {confirmation}: ") != confirmation:
            raise ValueError("Write cancelled.")
        # Recheck identity and safety after the user had time to unplug devices.
        disk = validate_target(devices(), str(device), size, protected_devices(image))
        if fingerprint(disk) != identity:
            raise ValueError("Drive changed after selection. Run again.")
        mounts = {m for n in descendants(disk) for m in (n.get("mountpoints") or []) if m}
        for mount in sorted(mounts, key=len, reverse=True):
            subprocess.run(["umount", "--", mount], check=True)
        disk = validate_target(devices(), str(device), size, protected_devices(image))
        if fingerprint(disk) != identity or any(m for n in descendants(disk) for m in (n.get("mountpoints") or [])):
            raise ValueError("Drive changed or is still mounted.")
        # O_EXCL claims the entire block device; mounted/in-use devices fail EBUSY.
        fd = os.open(device, os.O_RDWR | os.O_EXCL)
        with os.fdopen(fd, "r+b", buffering=0) as target:
            actual = os.fstat(target.fileno())
            if f"{os.major(actual.st_rdev)}:{os.minor(actual.st_rdev)}" != disk["maj:min"]:
                raise ValueError("A different device was opened; write cancelled.")
            length = struct.unpack("Q", fcntl.ioctl(fd, 0x80081272, bytes(8)))[0]  # BLKGETSIZE64
            if length != disk["size"]:
                raise ValueError("Drive size changed; write cancelled.")
            def flush():
                os.fsync(fd)
                fcntl.ioctl(fd, 0x1261)  # BLKFLSBUF: invalidate block cache before read-back
                print("\nReading back and verifying SHA256...", flush=True)
            copy_and_verify(source, target, size, length, digest, flush,
                            lambda percent: print(f"\rWriting: {percent:3d}%", end="", flush=True))
        result = subprocess.run(["blockdev", "--rereadpt", str(device)], check=False)
        if result.returncode:
            print("The partition table will refresh after the USB drive is reconnected.")
        print("Done. SHA256 matches. You can remove the USB drive. Boot: UEFI x64, Secure Boot off.")


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, subprocess.CalledProcessError, KeyboardInterrupt) as error:
        print(f"\nError/cancelled: {error}", file=sys.stderr)
        sys.exit(1)
