#!/bin/bash
set -e
source "$HOME/.cargo/env" 2>/dev/null || true
export PATH="$HOME/.cargo/bin:$PATH"

# Авто-определение корня в самом скрипте сборки
BUILD_SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo ">>> [1/3] Сборка Ядра и Приложения (ELF -> Static)..."
cd "$BUILD_SCRIPT_DIR/kernel" && cargo build --release
cd "$BUILD_SCRIPT_DIR/app" && cargo build --release

echo ">>> [2/3] Экстракция чистых секций (Flat Binary)..."
OBJCOPY=$(find $(rustc --print sysroot) -name llvm-objcopy | head -n 1)

$OBJCOPY -O binary -j .text -j .rodata -j .data "$BUILD_SCRIPT_DIR/kernel/target/x86_64-unknown-none/release/kernel" "$BUILD_SCRIPT_DIR/bootloader/src/kernel.bin"
$OBJCOPY -O binary -j .text -j .rodata -j .data "$BUILD_SCRIPT_DIR/app/target/x86_64-unknown-none/release/app" "$BUILD_SCRIPT_DIR/bootloader/src/app.bin"

echo ">>> [3/3] Сборка Загрузчика UEFI..."
cd "$BUILD_SCRIPT_DIR/bootloader" && cargo build --release --target x86_64-unknown-uefi
cp "$BUILD_SCRIPT_DIR/bootloader/target/x86_64-unknown-uefi/release/bootloader.efi" "$BUILD_SCRIPT_DIR/usb_root/EFI/BOOT/BOOTX64.EFI"

echo ">>> Успешно! Flat Binary собран."
