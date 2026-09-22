#!/bin/bash
# 02_build.sh — сборка ядра, пользовательских приложений и UEFI-загрузчика.
# Полный протокол сборки пишется в code_handoff/build.log
# (перезаписывается при каждом запуске).

set -o pipefail

BUILD_SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LOG_DIR="$BUILD_SCRIPT_DIR/code_handoff"
LOG_FILE="$LOG_DIR/build.log"

# Весь вывод (включая stderr cargo) идёт и на экран, и в лог.
mkdir -p "$LOG_DIR"
: > "$LOG_FILE"
exec > >(tee -a "$LOG_FILE") 2>&1

source "$HOME/.cargo/env" 2>/dev/null || true
export PATH="$HOME/.cargo/bin:$PATH"

START_TS=$(date +%s)
STEP="инициализация"
FAILED=0

echo "=========================================================="
echo "СБОРКА: $(date '+%Y-%m-%d %H:%M:%S')"
echo "Каталог: $BUILD_SCRIPT_DIR"
echo "Лог:     $LOG_FILE"
echo "=========================================================="

fail() {
    FAILED=1
    echo
    echo "!!! ОШИБКА на шаге: $STEP"
    finish
}

finish() {
    local elapsed=$(( $(date +%s) - START_TS ))
    echo
    echo "=========================================================="
    if [ "$FAILED" -eq 0 ]; then
        echo "РЕЗУЛЬТАТ: УСПЕШНО"
        echo "Время сборки: ${elapsed} с"
        echo "Артефакты:    usb_root/ (ядро, приложения, EFI/BOOT/BOOTX64.EFI)"
    else
        echo "РЕЗУЛЬТАТ: ОШИБКА"
        echo "Сбойный шаг:  $STEP"
        echo "Время:        ${elapsed} с"
    fi
    echo "Протокол:     $LOG_FILE"
    echo "=========================================================="
    # Даём tee дописать буфер перед выходом.
    exec 1>&- 2>&-
    wait
    exit "$FAILED"
}

trap fail ERR
set -e

# Каталог крейта : имя собранного бинарника : имя файла в usb_root
USER_CRATES=(
    "kernel:kernel:kernel.elf"
    "app:app:app.elf"
    "app2:app2:app2.elf"
    "clock:clock:clock.elf"
    "dzen-clock:dzen-clock:dzenclk.elf"
    "ping:ping:ping.elf"
    "pong:pong:pong.elf"
)

echo
echo ">>> [1/3] Сборка Ядра и Приложений (ELF)..."
for entry in "${USER_CRATES[@]}"; do
    crate_dir=${entry%%:*}
    STEP="cargo build --release в $crate_dir"
    echo "    --- $crate_dir ---"
    cd "$BUILD_SCRIPT_DIR/$crate_dir"
    cargo build --release
done

echo
echo ">>> [2/3] Сборка Загрузчика UEFI (ELF Parser)..."
STEP="cargo build --release --target x86_64-unknown-uefi в bootloader"
cd "$BUILD_SCRIPT_DIR/bootloader"
cargo build --release --target x86_64-unknown-uefi

echo
echo ">>> [3/3] Подготовка файлов EFI/ELF в каталоге usb_root..."
STEP="раскладка артефактов в usb_root"
mkdir -p "$BUILD_SCRIPT_DIR/usb_root/EFI/BOOT"

for entry in "${USER_CRATES[@]}"; do
    crate_dir=${entry%%:*}
    rest=${entry#*:}
    bin_name=${rest%%:*}
    out_name=${rest#*:}

    src="$BUILD_SCRIPT_DIR/$crate_dir/target/x86_64-unknown-none/release/$bin_name"
    dst="$BUILD_SCRIPT_DIR/usb_root/$out_name"

    STEP="копирование $crate_dir -> usb_root/$out_name"
    if [ ! -f "$src" ]; then
        echo "    ОТСУТСТВУЕТ: $src"
        fail
    fi
    cp "$src" "$dst"
    printf '    %-14s -> usb_root/%-14s (%s байт)\n' "$crate_dir" "$out_name" "$(stat -c%s "$dst")"
done

STEP="копирование загрузчика в usb_root/EFI/BOOT"
BOOT_SRC="$BUILD_SCRIPT_DIR/bootloader/target/x86_64-unknown-uefi/release/bootloader.efi"
if [ ! -f "$BOOT_SRC" ]; then
    echo "    ОТСУТСТВУЕТ: $BOOT_SRC"
    fail
fi
cp "$BOOT_SRC" "$BUILD_SCRIPT_DIR/usb_root/EFI/BOOT/BOOTX64.EFI"
printf '    %-14s -> usb_root/%-14s (%s байт)\n' "bootloader" "EFI/BOOT/BOOTX64.EFI" \
    "$(stat -c%s "$BUILD_SCRIPT_DIR/usb_root/EFI/BOOT/BOOTX64.EFI")"

trap - ERR
finish
