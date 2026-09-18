#!/bin/bash
set -e
source "$HOME/.cargo/env" 2>/dev/null || true
export PATH="$HOME/.cargo/bin:$PATH"

BUILD_SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo ">>> [1/3] Сборка Ядра и Приложения (ELF)..."
cd "$BUILD_SCRIPT_DIR/kernel" && cargo build --release
cd "$BUILD_SCRIPT_DIR/app" && cargo build --release

echo ">>> [2/3] Сборка Загрузчика UEFI (ELF Parser)..."
cd "$BUILD_SCRIPT_DIR/bootloader" && cargo build --release --target x86_64-unknown-uefi

echo ">>> [3/3] Размещение ELF файлов на файловой системе FAT32..."
cp "$BUILD_SCRIPT_DIR/kernel/target/x86_64-unknown-none/release/kernel" "$BUILD_SCRIPT_DIR/usb_root/kernel.elf"
cp "$BUILD_SCRIPT_DIR/app/target/x86_64-unknown-none/release/app" "$BUILD_SCRIPT_DIR/usb_root/app.elf"
cp "$BUILD_SCRIPT_DIR/bootloader/target/x86_64-unknown-uefi/release/bootloader.efi" "$BUILD_SCRIPT_DIR/usb_root/EFI/BOOT/BOOTX64.EFI"

echo ">>> Готово! Файлы лежат на виртуальной флешке."
