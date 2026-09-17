#!/bin/bash
set -e

source "$HOME/.cargo/env" 2>/dev/null || true

echo ">>> [1/4] Компиляция Ядра и Приложения (ELF)..."
cd kernel && cargo build --release && cd ..
cd app && cargo build --release && cd ..

echo ">>> [2/4] Конвертация в плоские бинарники (Raw Binaries)..."
OBJCOPY=$(find $(rustc --print sysroot) -name llvm-objcopy | head -n 1)
$OBJCOPY -O binary kernel/target/x86_64-unknown-none/release/kernel bootloader/src/kernel.bin
$OBJCOPY -O binary app/target/x86_64-unknown-none/release/app bootloader/src/app.bin

echo ">>> [3/4] Компиляция Загрузчика (PE/COFF)..."
cd bootloader && cargo build --release --target x86_64-unknown-uefi && cd ..

echo ">>> [4/4] Сборка USB-образа..."
cp bootloader/target/x86_64-unknown-uefi/release/bootloader.efi usb_root/EFI/BOOT/BOOTX64.EFI

echo "=========================================================="
echo "СИСТЕМА СОБРАНА УСПЕШНО!"
echo "Артефакты находятся в usb_root/EFI/BOOT"
echo "=========================================================="
