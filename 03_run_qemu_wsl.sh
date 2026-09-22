#!/usr/bin/env bash
# Запуск Windows QEMU из WSL через interop после ./02_build.sh.
set -euo pipefail

RUN_SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

fail() {
    printf 'ОШИБКА: %s\n' "$*" >&2
    exit 1
}

command -v wslpath >/dev/null 2>&1 || fail "Запустите этот скрипт в WSL с включённым Windows interop."

# Как в MSYS2 launcher: сначала UCRT64, затем MinGW64.
# QEMU позволяет явно задать путь к .exe в формате WSL.
QEMU_BIN="${QEMU:-}"
if [[ -z "$QEMU_BIN" ]]; then
    for windows_path in \
        'C:\msys64\ucrt64\bin\qemu-system-x86_64.exe' \
        'C:\msys64\mingw64\bin\qemu-system-x86_64.exe' \
        'C:\Program Files\qemu\qemu-system-x86_64.exe'; do
        candidate="$(wslpath -u "$windows_path")"
        if [[ -x "$candidate" ]]; then
            QEMU_BIN="$candidate"
            break
        fi
    done
    if [[ -z "$QEMU_BIN" ]]; then
        QEMU_BIN="$(command -v qemu-system-x86_64.exe || true)"
    fi
fi
[[ -n "$QEMU_BIN" ]] || fail "Windows QEMU не найден. Укажите QEMU=/mnt/c/путь/qemu-system-x86_64.exe."
QEMU_BIN="$(command -v -- "$QEMU_BIN")" || fail "Не найден исполняемый файл QEMU: ${QEMU:-qemu-system-x86_64.exe}"

[[ -f "$RUN_SCRIPT_DIR/OVMF.fd" ]] || fail "Поместите UEFI-прошивку OVMF.fd рядом со скриптом."
for artifact in EFI/BOOT/BOOTX64.EFI kernel.elf; do
    [[ -f "$RUN_SCRIPT_DIR/usb_root/$artifact" ]] || fail "Нет usb_root/$artifact. Сначала выполните ./02_build.sh."
done

# WSL не переводит аргументы-пути для Windows-программ автоматически.
FIRMWARE_PATH="$(wslpath -w "$RUN_SCRIPT_DIR/OVMF.fd")"
USB_ROOT_PATH="$(wslpath -w "$RUN_SCRIPT_DIR/usb_root")"
# В -drive запятая разделяет параметры; запятые в имени удваиваются.
USB_ROOT_PATH="${USB_ROOT_PATH//,/,,}"

printf 'Запуск MIND CORE в Windows QEMU из WSL: %s\n' "$QEMU_BIN"
exec "$QEMU_BIN" \
    -bios "$FIRMWARE_PATH" \
    -drive "format=raw,file=fat:rw:$USB_ROOT_PATH" \
    -m 512 -smp 4,sockets=1,cores=4,threads=1 \
    -serial stdio -rtc base=localtime \
    "$@"
