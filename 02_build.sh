#!/bin/bash
set -o pipefail
BUILD_SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LOG_DIR="$BUILD_SCRIPT_DIR/code_handoff"
LOG_FILE="$LOG_DIR/build.log"
mkdir -p "$LOG_DIR"
: > "$LOG_FILE"
exec > >(tee -a "$LOG_FILE") 2>&1
source "$HOME/.cargo/env" 2>/dev/null || true
export PATH="$HOME/.cargo/bin:$PATH"

USER_CRATES=(
    "kernel:kernel:kernel.elf"
    "app:app:app.elf"
    "app2:app2:app2.elf"
    "clock:clock:clock.elf"
    "dzen-clock:dzen-clock:dzenclk.elf"
    "ping:ping:ping.elf"
    "pong:pong:pong.elf"
    "rtc:rtc:rtc.elf"
    "ps2_kbd:ps2_kbd:ps2_kbd.elf"
)

for entry in "${USER_CRATES[@]}"; do
    crate_dir=${entry%%:*}
    cd "$BUILD_SCRIPT_DIR/$crate_dir"
    cargo build --release
done

cd "$BUILD_SCRIPT_DIR/bootloader"
cargo build --release --target x86_64-unknown-uefi

mkdir -p "$BUILD_SCRIPT_DIR/usb_root/EFI/BOOT"
for entry in "${USER_CRATES[@]}"; do
    crate_dir=${entry%%:*}
    rest=${entry#*:}
    bin_name=${rest%%:*}
    out_name=${rest#*:}
    cp "$BUILD_SCRIPT_DIR/$crate_dir/target/x86_64-unknown-none/release/$bin_name" "$BUILD_SCRIPT_DIR/usb_root/$out_name"
done
cp "$BUILD_SCRIPT_DIR/bootloader/target/x86_64-unknown-uefi/release/bootloader.efi" "$BUILD_SCRIPT_DIR/usb_root/EFI/BOOT/BOOTX64.EFI"
