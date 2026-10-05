#!/usr/bin/env bash
# Run Windows QEMU from WSL via interop after ./02_build.sh.
set -euo pipefail

RUN_SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

fail() {
    printf 'ERROR: %s\n' "$*" >&2
    exit 1
}

command -v wslpath >/dev/null 2>&1 || fail "Run this script in WSL with Windows interop enabled."

# As in the MSYS2 launcher: UCRT64 first, then MinGW64.
# QEMU lets you set the .exe path explicitly in WSL format.
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
[[ -n "$QEMU_BIN" ]] || fail "Windows QEMU not found. Set QEMU=/mnt/c/path/to/qemu-system-x86_64.exe."
QEMU_BIN="$(command -v -- "$QEMU_BIN")" || fail "QEMU executable not found: ${QEMU:-qemu-system-x86_64.exe}"

[[ -f "$RUN_SCRIPT_DIR/OVMF.fd" ]] || fail "Place the OVMF.fd UEFI firmware next to this script."
for artifact in EFI/BOOT/BOOTX64.EFI kernel.elf; do
    [[ -f "$RUN_SCRIPT_DIR/usb_root/$artifact" ]] || fail "Missing usb_root/$artifact. Run ./02_build.sh first."
done

# WSL does not translate path arguments for Windows programs automatically.
FIRMWARE_PATH="$(wslpath -w "$RUN_SCRIPT_DIR/OVMF.fd")"
USB_ROOT_PATH="$(wslpath -w "$RUN_SCRIPT_DIR/usb_root")"
# In -drive a comma separates options; commas in the name are doubled.
USB_ROOT_PATH="${USB_ROOT_PATH//,/,,}"

printf 'Starting MIND CORE in Windows QEMU from WSL: %s\n' "$QEMU_BIN"
# Sound through Windows (DirectSound) on the AC97 card audio_gw drives (MIND_AUDIO=none: no card); a VirtIO network
# card (MIND_NET=none: none); RDRAND for the TLS and key services (MIND_CPU=<model> to change it).
AUDIO=()
[[ "${MIND_AUDIO:-dsound}" == none ]] || AUDIO=(-audiodev "${MIND_AUDIO:-dsound},id=snd0" -device AC97,audiodev=snd0)
NET=()
[[ "${MIND_NET:-user}" == none ]] || NET=(-nic "user,model=virtio-net-pci")
# Pointer: a VirtIO tablet, an absolute device, so QEMU needs no pointer grab (MIND_POINTER=ps2: the PS/2 mouse only).
POINTER=()
[[ "${MIND_POINTER:-tablet}" == ps2 ]] || POINTER=(-device virtio-tablet-pci)
exec "$QEMU_BIN" \
    -bios "$FIRMWARE_PATH" \
    -drive "format=raw,file=fat:rw:$USB_ROOT_PATH" \
    -m 512 -smp 4,sockets=1,cores=4,threads=1 \
    -cpu "${MIND_CPU:-qemu64,+rdrand}" \
    -serial stdio -rtc base=localtime \
    "${AUDIO[@]}" "${NET[@]}" "${POINTER[@]}" \
    "$@"
