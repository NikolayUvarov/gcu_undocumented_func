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
        raise ValueError("Накопитель не найден или имеет неоднозначную топологию.")
    disk = candidates[0]
    if disk["type"] != "disk" or disk.get("tran") != "usb":
        raise ValueError("Нужен целый USB-диск, например /dev/sdb, а не раздел /dev/sdb1.")
    if disk["ro"] or disk["log-sec"] != 512 or disk["size"] < size:
        raise ValueError("Диск защищён от записи, имеет сектор не 512 байт или меньше образа.")
    for node in descendants(disk):
        mounts = node.get("mountpoints") or []
        if node["maj:min"] in protected or any(m in ("/", "/boot", "/boot/efi", "/usr", "/var", "/home") for m in mounts):
            raise ValueError("Это системный диск или на нём находятся образ/скрипт/текущий каталог.")
        if node is not disk and node["type"] != "part":
            raise ValueError("Диск используется LVM/RAID/crypt или другим составным устройством.")
        if "[SWAP]" in mounts:
            raise ValueError("На диске используется swap. Запись запрещена.")
    return disk


def protected_devices(image):
    protected = set()
    for path in [Path(p) for p in ("/", "/boot", "/boot/efi", "/usr", "/var", "/home")] + [image, ROOT, Path.cwd()]:
        if path.exists():
            value = subprocess.check_output(
                ["findmnt", "--noheadings", "--output", "MAJ:MIN", "--target", str(path.resolve())],
                text=True).strip()
            if not value:
                raise ValueError(f"Не удалось определить файловую систему {path}.")
            protected.update(value.splitlines())
    return protected


def image_info(source):
    source.seek(0, os.SEEK_END)
    size = source.tell()
    source.seek(0)
    mbr = source.read(512)
    if size % 512 or len(mbr) != 512 or mbr[510:] != b"\x55\xaa" or mbr[450] != 0xef:
        raise ValueError("Ожидается RAW-образ MIND CORE с MBR/UEFI-разделом.")
    start, sectors = struct.unpack_from("<II", mbr, 454)
    if start == 0 or sectors == 0 or (start + sectors) * 512 > size:
        raise ValueError("Раздел образа выходит за границы файла.")
    source.seek(start * 512)
    boot = source.read(512)
    if len(boot) != 512 or boot[510:] != b"\x55\xaa" or boot[43:54] != b"MIND CORE  " or boot[54:62] != b"FAT16   ":
        raise ValueError("В образе отсутствует FAT16-раздел MIND CORE.")
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
            raise OSError("Неполная запись на накопитель.")
        view = view[count:]


def copy_and_verify(source, target, size, disk_size, expected, flush, progress=lambda n: None):
    """Stream core, also exercised with ordinary temporary files in tests."""
    if size <= 0 or size % 512 or disk_size < size or disk_size % 512:
        raise ValueError("Недопустимый размер образа/накопителя.")
    source.seek(0)
    target.seek(0)
    copied = 0
    while copied < size:
        data = source.read(min(CHUNK, size - copied))
        if not data:
            raise OSError("Образ изменился/оборвался во время записи.")
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
            raise OSError("Не удалось прочитать записанный образ целиком.")
        digest.update(data)
        remaining -= len(data)
    if digest.hexdigest() != expected:
        raise OSError("SHA256 не совпал: запись не прошла проверку. Флешка не готова.")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--list", action="store_true", help="показать USB-диски без записи")
    parser.add_argument("--device", help="целый диск: /dev/sdX или /dev/disk/by-id/...")
    parser.add_argument("--image", type=Path, default=ROOT / "dist/mind-core-usb.img")
    parser.add_argument("--check", action="store_true", help="проверить выбор без размонтирования/записи")
    args = parser.parse_args()
    if not sys.platform.startswith("linux"):
        raise ValueError("Этот скрипт предназначен для Linux. В Windows используйте .ps1.")
    tree = devices()
    if args.list or not args.device:
        print("USB-диски (PATH | SIZE GiB | MODEL | SERIAL):")
        count = 0
        for disk in tree:
            if disk["type"] == "disk" and disk.get("tran") == "usb":
                print(f"{disk['path']} | {disk['size'] / 2**30:.2f} | {disk.get('model')} | {disk.get('serial')}")
                count += 1
        if not count:
            print("Не обнаружены. Для флешки, подключённой к Windows, используйте Windows-скрипт.")
        if not args.list:
            parser.error("укажите --device /dev/sdX; запись автоматически не выбирает диск")
        return
    device = Path(args.device).resolve(strict=True)
    if not stat.S_ISBLK(device.stat().st_mode):
        raise ValueError("--device должен указывать на блочное устройство, не файл.")
    image = args.image.resolve(strict=True)
    if not image.is_file():
        raise ValueError("--image должен указывать на обычный файл образа.")
    with image.open("rb") as source:
        size, digest = image_info(source)
        disk = validate_target(tree, str(device), size, protected_devices(image))
        identity = fingerprint(disk)
        print(f"Образ: {image}\nРазмер: {size} байт\nSHA256: {digest}\n"
              f"USB: {device} | {disk.get('model')} | {disk.get('serial')} | {disk['size']} байт")
        mounts = {m for n in descendants(disk) for m in (n.get("mountpoints") or []) if m}
        print("Будут размонтированы: " + (", ".join(sorted(mounts)) or "нет"))
        if args.check:
            print("Проверка пройдена. Запись не выполнялась.")
            return
        if os.geteuid() != 0:
            raise ValueError("Для записи запустите скрипт через sudo.")
        if not sys.stdin.isatty():
            raise ValueError("Подтверждение записи нужно вводить в интерактивном терминале.")
        confirmation = f"ERASE {device}"
        print("ВСЕ ДАННЫЕ НА ВЫБРАННОМ USB-ДИСКЕ БУДУТ ПОТЕРЯНЫ.")
        if input(f"Введите {confirmation}: ") != confirmation:
            raise ValueError("Запись отменена.")
        # Recheck identity and safety after the user had time to unplug devices.
        disk = validate_target(devices(), str(device), size, protected_devices(image))
        if fingerprint(disk) != identity:
            raise ValueError("Накопитель изменился после выбора. Повторите запуск.")
        mounts = {m for n in descendants(disk) for m in (n.get("mountpoints") or []) if m}
        for mount in sorted(mounts, key=len, reverse=True):
            subprocess.run(["umount", "--", mount], check=True)
        disk = validate_target(devices(), str(device), size, protected_devices(image))
        if fingerprint(disk) != identity or any(m for n in descendants(disk) for m in (n.get("mountpoints") or [])):
            raise ValueError("Накопитель изменился или остался смонтирован.")
        # O_EXCL claims the entire block device; mounted/in-use devices fail EBUSY.
        fd = os.open(device, os.O_RDWR | os.O_EXCL)
        with os.fdopen(fd, "r+b", buffering=0) as target:
            actual = os.fstat(target.fileno())
            if f"{os.major(actual.st_rdev)}:{os.minor(actual.st_rdev)}" != disk["maj:min"]:
                raise ValueError("Открыто другое устройство; запись отменена.")
            length = struct.unpack("Q", fcntl.ioctl(fd, 0x80081272, bytes(8)))[0]  # BLKGETSIZE64
            if length != disk["size"]:
                raise ValueError("Размер накопителя изменился; запись отменена.")
            def flush():
                os.fsync(fd)
                fcntl.ioctl(fd, 0x1261)  # BLKFLSBUF: invalidate block cache before read-back
                print("\nПовторное чтение и проверка SHA256...", flush=True)
            copy_and_verify(source, target, size, length, digest, flush,
                            lambda percent: print(f"\rЗапись: {percent:3d}%", end="", flush=True))
        result = subprocess.run(["blockdev", "--rereadpt", str(device)], check=False)
        if result.returncode:
            print("Таблица разделов обновится после переподключения флешки.")
        print("Готово. SHA256 совпал. Можно извлечь флешку. Загрузка: UEFI x64, Secure Boot off.")


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, subprocess.CalledProcessError, KeyboardInterrupt) as error:
        print(f"\nОшибка/отмена: {error}", file=sys.stderr)
        sys.exit(1)
