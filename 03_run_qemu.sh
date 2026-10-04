#!/usr/bin/env bash
# Run MIND CORE in QEMU on Linux after ./02_build.sh (same VM settings as the Windows and WSL launchers).
set -euo pipefail

RUN_SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

fail() {
    printf 'ERROR: %s\n' "$*" >&2
    exit 1
}

QEMU_BIN="${QEMU:-qemu-system-x86_64}"
command -v -- "$QEMU_BIN" >/dev/null 2>&1 || fail "QEMU not found. Install qemu-system-x86 or set QEMU=/path/to/qemu-system-x86_64."

# Firmware: OVMF.fd next to this script, else the split OVMF_CODE/OVMF_VARS of the distribution (pflash, read-only
# code and a private copy of the variables).
FIRMWARE=()
if [[ -f "$RUN_SCRIPT_DIR/OVMF.fd" ]]; then
    FIRMWARE=(-bios "$RUN_SCRIPT_DIR/OVMF.fd")
else
    for code in /usr/share/OVMF/OVMF_CODE_4M.fd /usr/share/OVMF/OVMF_CODE.fd /usr/share/edk2/ovmf/OVMF_CODE.fd /usr/share/qemu/OVMF_CODE.fd; do
        vars="${code/CODE/VARS}"
        if [[ -f "$code" && -f "$vars" ]]; then
            local_vars="$(mktemp --suffix=-OVMF_VARS.fd)"
            cp "$vars" "$local_vars"
            FIRMWARE=(-drive "if=pflash,format=raw,readonly=on,file=$code" -drive "if=pflash,format=raw,file=$local_vars")
            break
        fi
    done
fi
[[ ${#FIRMWARE[@]} -gt 0 ]] || fail "No UEFI firmware: place OVMF.fd next to this script or install the ovmf package."
for artifact in EFI/BOOT/BOOTX64.EFI kernel.elf; do
    [[ -f "$RUN_SCRIPT_DIR/usb_root/$artifact" ]] || fail "Missing usb_root/$artifact. Run ./02_build.sh first."
done

# In -drive a comma separates options; commas in the path are doubled.
USB_ROOT_PATH="${RUN_SCRIPT_DIR//,/,,}/usb_root"

printf 'Starting MIND CORE in QEMU: %s\n' "$QEMU_BIN"
exec "$QEMU_BIN" \
    "${FIRMWARE[@]}" \
    -drive "format=raw,file=fat:rw:$USB_ROOT_PATH" \
    -m 512 -smp 4,sockets=1,cores=4,threads=1 \
    -serial stdio -rtc base=localtime \
    "$@"
