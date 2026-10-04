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

START_TS=$(date +%s)
STEP="initialization"
FAILED=0

fail() {
    FAILED=1
    echo
    echo "!!! ERROR at step: $STEP"
    finish
}

finish() {
    local elapsed=$(( $(date +%s) - START_TS ))
    echo
    echo "=========================================================="
    if [ "$FAILED" -eq 0 ]; then
        echo "RESULT: SUCCESS"
        echo "Build time: ${elapsed} s"
    else
        echo "RESULT: FAILURE"
        echo "Failed step: $STEP"
    fi
    echo "=========================================================="
    exec 1>&- 2>&-
    wait
    exit "$FAILED"
}

trap fail ERR
set -e

USER_CRATES=(
    "kernel:kernel:kernel.elf"
    "init:init:init.elf"
    "shell:shell:shell.elf"
    "app:app:app.elf"
    "app2:app2:app2.elf"
    "clock:clock:clock.elf"
    "dzen-clock:dzen-clock:dzen-clock.elf"
    "ping:ping:ping.elf"
    "pong:pong:pong.elf"
    "rtc:rtc:rtc.elf"
    "ps2_kbd:ps2_kbd:ps2_kbd.elf"
    "compositor:compositor:compositor.elf"
    "ata:ata:ata.elf"
    "ahci:ahci:ahci.elf"
    "usb_storage:usb_storage:usb_storage.elf"
    "vfs_server:vfs_server:vfs_server.elf"
    "loader:loader:loader.elf"
    "audio_gw:audio_gw:audio_gw.elf"
    "tts:tts:tts.elf"
    "sysmon:sysmon:sysmon.elf"
    "say:say:say.elf"
    "listen:listen:listen.elf"
    "files:files:files.elf"
    "keys:keys:keys.elf"
    "view:view:view.elf"
    "uptime:uptime:uptime.elf"
    "beep:beep:beep.elf"
)

echo ">>> [1/3] Building the kernel and apps (ELF)..."
for entry in "${USER_CRATES[@]}"; do
    crate_dir=${entry%%:*}
    STEP="cargo build --release in $crate_dir"
    cd "$BUILD_SCRIPT_DIR/$crate_dir"
    cargo build --release
done

echo ">>> [2/3] Building the UEFI bootloader..."
STEP="cargo build --release --target x86_64-unknown-uefi in bootloader"
cd "$BUILD_SCRIPT_DIR/bootloader"
cargo build --release --target x86_64-unknown-uefi

echo ">>> [3/3] Staging EFI/ELF files..."
STEP="staging artifacts"
mkdir -p "$BUILD_SCRIPT_DIR/usb_root/EFI/BOOT"

for entry in "${USER_CRATES[@]}"; do
    crate_dir=${entry%%:*}
    rest=${entry#*:}
    bin_name=${rest%%:*}
    out_name=${rest#*:}
    cp "$BUILD_SCRIPT_DIR/$crate_dir/target/x86_64-unknown-none/release/$bin_name" "$BUILD_SCRIPT_DIR/usb_root/$out_name"
done
cp "$BUILD_SCRIPT_DIR/bootloader/target/x86_64-unknown-uefi/release/bootloader.efi" "$BUILD_SCRIPT_DIR/usb_root/EFI/BOOT/BOOTX64.EFI"

trap - ERR
finish
