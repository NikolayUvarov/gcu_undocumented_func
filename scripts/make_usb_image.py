#!/usr/bin/env python3
"""Build MIND CORE and package a raw UEFI USB image; never access physical disks."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
FILES = ("EFI/BOOT/BOOTX64.EFI", "kernel.elf", "app.elf", "app2.elf", "clock.elf", "dzenclk.elf")
SECTOR = 512


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
    raise ValueError("qemu-img не найден. Установите QEMU или укажите --qemu-img /путь/qemu-img.")


def qemu_path(path, executable):
    """Windows QEMU under WSL/MSYS needs a Windows path, including drive letters."""
    path = str(path.resolve())
    if executable.lower().endswith(".exe") and os.name != "nt":
        converter = shutil.which("wslpath") or shutil.which("cygpath")
        if not converter:
            raise ValueError("Для Windows QEMU нужен wslpath/cygpath; либо используйте native qemu-img.")
        path = subprocess.check_output([converter, "-w", path], text=True).strip()
    return path


def read_payloads(source):
    payloads = {}
    for name in FILES:
        file = source / name
        if not file.is_file():
            raise ValueError(f"Нет {file}. Выполните сборку без --no-build.")
        # The current bootloader's ELF read buffer is 4 MiB.
        if not 0 < file.stat().st_size <= 4 * 1024 * 1024:
            raise ValueError(f"Недопустимый размер {file}: ожидается 1..4194304 байт.")
        data = file.read_bytes()
        if name.endswith(".elf"):
            if len(data) < 64 or data[:6] != b"\x7fELF\x02\x01" or data[18:20] != b">\x00":
                raise ValueError(f"{file} не является ELF64 x86-64 little-endian.")
        elif data[:2] != b"MZ":
            raise ValueError(f"{file} не является PE/EFI-приложением.")
        payloads[name] = data
    return payloads


def check_image(image, payloads, mark_esp=False):
    """Check MBR/FAT16 geometry and all packaged file contents independently of QEMU.

    Only reads regular image files; mark_esp changes the generated partition's
    type to the UEFI-defined MBR ESP type (0xEF), preserving its FAT16 geometry.
    """
    def require(condition, message):
        if not condition:
            raise ValueError(f"Некорректный USB-образ: {message}")

    with image.open("r+b" if mark_esp else "rb") as disk:
        mbr = disk.read(SECTOR)
        require(len(mbr) == SECTOR and mbr[510:] == b"\x55\xaa", "сигнатура MBR")
        require(not any(mbr[462:510]), "ожидался один раздел")
        require(mbr[450] in (0x04, 0x06, 0x0e, 0xef), "тип раздела FAT16/ESP")
        start, length = struct.unpack_from("<II", mbr, 454)
        require(start > 0 and length > 0 and (start + length) * SECTOR <= image.stat().st_size,
                "раздел выходит за границы образа")
        disk.seek(start * SECTOR)
        boot = disk.read(SECTOR)
        require(len(boot) == SECTOR and boot[510:] == b"\x55\xaa", "FAT boot sector")
        bps, spc, reserved, fats, roots, total16 = struct.unpack_from("<HBHBHH", boot, 11)
        fat_sectors = struct.unpack_from("<H", boot, 22)[0]
        total = total16 or struct.unpack_from("<I", boot, 32)[0]
        require(bps == SECTOR and spc > 0 and spc & (spc - 1) == 0 and reserved > 0
                and fats == 2 and roots > 0 and fat_sectors > 0 and total == length,
                "параметры FAT16")
        root_sectors = (roots * 32 + SECTOR - 1) // SECTOR
        first_data = reserved + fats * fat_sectors + root_sectors
        clusters = (total - first_data) // spc
        require(4085 <= clusters < 65525 and (clusters + 2) * 2 <= fat_sectors * SECTOR,
                "число кластеров FAT16")
        disk.seek((start + reserved) * SECTOR)
        fat = disk.read(fat_sectors * SECTOR)
        require(fat == disk.read(fat_sectors * SECTOR), "копии FAT различаются")
        root = disk.read(root_sectors * SECTOR)
        cluster_size = spc * SECTOR

        def chain(first):
            result, visited = bytearray(), set()
            while first < 0xfff8:
                require(2 <= first < clusters + 2 and first not in visited, "цепочка кластеров")
                visited.add(first)
                require(len(visited) * cluster_size <= 4 * 1024 * 1024 + cluster_size,
                        "слишком длинная цепочка")
                disk.seek((start + first_data + (first - 2) * spc) * SECTOR)
                result.extend(disk.read(cluster_size))
                first = struct.unpack_from("<H", fat, first * 2)[0]
            return bytes(result)

        def lookup(directory, part):
            stem, _, extension = part.upper().partition(".")
            short_name = (stem.ljust(8) + extension.ljust(3)).encode("ascii")
            for offset in range(0, len(directory), 32):
                entry = directory[offset:offset + 32]
                if entry[0] == 0:
                    break
                if entry[0] != 0xe5 and not entry[11] & 8 and entry[:11] == short_name:
                    return entry
            raise ValueError(f"В USB-образе отсутствует {part}")

        for name, expected in payloads.items():
            directory = root
            parts = name.split("/")
            for part in parts[:-1]:
                entry = lookup(directory, part)
                require(entry[11] & 0x10, f"{part} не каталог")
                directory = chain(struct.unpack_from("<H", entry, 26)[0])
            entry = lookup(directory, parts[-1])
            size = struct.unpack_from("<I", entry, 28)[0]
            require(not entry[11] & 0x10 and size == len(expected), f"размер {name}")
            actual = chain(struct.unpack_from("<H", entry, 26)[0])[:size]
            require(actual == expected, f"содержимое {name} не совпало с результатом сборки")
        if mark_esp:
            disk.seek(450)
            disk.write(b"\xef")
            disk.flush()
            os.fsync(disk.fileno())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=ROOT / "dist/mind-core-usb.img",
                        help="путь к .img (по умолчанию dist/mind-core-usb.img в проекте)")
    parser.add_argument("--no-build", action="store_true", help="использовать уже собранный usb_root/")
    parser.add_argument("--force", action="store_true", help="заменить существующий файл образа")
    parser.add_argument("--qemu-img", default=os.environ.get("QEMU_IMG"), help="путь к qemu-img[.exe]")
    args = parser.parse_args()
    output = args.output.absolute()
    if output.suffix.lower() != ".img":
        raise ValueError("Выходной файл должен иметь расширение .img.")
    if output.is_symlink() or (output.exists() and not output.is_file()):
        raise ValueError("Выходной путь должен быть обычным файлом, не ссылкой или устройством.")
    if output.exists() and not args.force:
        raise ValueError(f"{output} уже существует. Для замены укажите --force.")
    qemu_img = find_qemu_img(args.qemu_img)
    if not args.no_build:
        print(">>> Сборка проекта...", flush=True)
        subprocess.run(["bash", str(ROOT / "02_build.sh")], cwd=ROOT, check=True)
    payloads = read_payloads(ROOT / "usb_root")
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
        temporary = work / "disk.img"
        descriptor = {"driver": "raw", "file": {
            "driver": "vvfat", "dir": qemu_path(source, qemu_img),
            "fat-type": 16, "floppy": False, "rw": False, "label": "MIND CORE",
        }}
        print(">>> Создание RAW-образа USB (MBR, FAT16, UEFI x64)...", flush=True)
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
        print(f">>> Готово: {output}\nРазмер: {output.stat().st_size} байт\n"
              f"SHA256: {digest.hexdigest()}\n"
              "Запишите .img на весь USB-накопитель в режиме RAW/DD.\n"
              "Загрузка: UEFI x64, Secure Boot выключен. Это не Legacy BIOS-образ.")


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f"Ошибка: {error}", file=sys.stderr)
        sys.exit(1)
