#!/usr/bin/env bash
# Runs MIND CORE for aarch64 on QEMU's `virt` machine with UEFI firmware (issues 201-205): the shell on the PL011 in
# this terminal, the screen (ramfb) in a window where there is a desktop session, a VirtIO network card, keyboard and
# tablet; the ECAM below 4 GiB (highmem=off); four CPUs.
#   ./03_run_qemu_aarch64.sh                 the build in aarch64_root/ (ARCH=aarch64 ./02_build.sh) as a FAT disk
#   ./03_run_qemu_aarch64.sh --image [file]  the USB image (./04_make_usb_image_aarch64.sh), as a USB stick on xHCI,
#                                            the way a board finds it; QEMU writes nothing to the file
# MIND_CPUS=n, MIND_NET=none, MIND_DISPLAY=none, MIND_AAVMF_CODE / MIND_AAVMF_VARS (firmware), QEMU=<binary>; further
# arguments go to QEMU. Leave QEMU with Ctrl+A X.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

fail() { printf 'ERROR: %s\n' "$*" >&2; exit 1; }

QEMU_BIN="${QEMU:-qemu-system-aarch64}"
command -v -- "$QEMU_BIN" >/dev/null 2>&1 || fail "QEMU for aarch64 not found. Debian/Ubuntu/WSL: sudo apt install qemu-system-arm qemu-efi-aarch64; Fedora: qemu-system-aarch64 edk2-aarch64; or set QEMU=/path/to/qemu-system-aarch64."

# Firmware: AAVMF (Debian, Ubuntu), edk2 (Fedora), or the one QEMU ships; the variables are a private copy.
CODE="${MIND_AAVMF_CODE:-}"; VARS_TEMPLATE="${MIND_AAVMF_VARS:-}"
if [[ -z "$CODE" ]]; then
    for pair in /usr/share/AAVMF/AAVMF_CODE.fd:/usr/share/AAVMF/AAVMF_VARS.fd \
                /usr/share/edk2/aarch64/QEMU_EFI-pflash.raw:/usr/share/edk2/aarch64/vars-template-pflash.raw \
                /usr/share/qemu/edk2-aarch64-code.fd:/usr/share/qemu/edk2-arm-vars.fd; do
        if [[ -f "${pair%%:*}" && -f "${pair##*:}" ]]; then CODE="${pair%%:*}"; VARS_TEMPLATE="${pair##*:}"; break; fi
    done
fi
[[ -n "$CODE" && -f "$CODE" && -f "$VARS_TEMPLATE" ]] || fail "No aarch64 UEFI firmware: install qemu-efi-aarch64 (Debian/Ubuntu) or edk2-aarch64 (Fedora), or set MIND_AAVMF_CODE and MIND_AAVMF_VARS."
VARS="$(mktemp --suffix=-AAVMF_VARS.fd)"; trap 'rm -f "$VARS"' EXIT; cp "$VARS_TEMPLATE" "$VARS"

# The disk: the build directory, or the USB image as a USB stick (snapshot: what the system writes stays in QEMU).
if [[ "${1:-}" == --image ]]; then
    shift
    IMAGE="$ROOT/dist/mind-core-usb-aarch64.img"
    if [[ $# -gt 0 && "$1" != -* ]]; then IMAGE="$(realpath "$1")"; shift; fi
    [[ -f "$IMAGE" ]] || fail "Missing $IMAGE. Run ./04_make_usb_image_aarch64.sh first."
    DISK=(-drive "if=none,id=stick,format=raw,snapshot=on,file=${IMAGE//,/,,}" -device qemu-xhci -device usb-storage,drive=stick,bootindex=1)
    WHAT="the USB image $IMAGE"
else
    for artifact in EFI/BOOT/BOOTAA64.EFI kernel.elf; do
        [[ -f "$ROOT/aarch64_root/$artifact" ]] || fail "Missing aarch64_root/$artifact. Run ARCH=aarch64 ./02_build.sh first."
    done
    DISK=(-drive "format=raw,file=fat:rw:${ROOT//,/,,}/aarch64_root")
    WHAT="aarch64_root/"
fi

# The screen in a window where there is a desktop session (WSLg too), else only the console on this terminal. The
# network card is a -device: QEMU gives a -nic card on virt no MSI-X.
DISPLAY_ARGS=(-display none)
[[ -n "${DISPLAY:-}${WAYLAND_DISPLAY:-}" && "${MIND_DISPLAY:-}" != none ]] && DISPLAY_ARGS=()
NET=()
[[ "${MIND_NET:-user}" == none ]] || NET=(-netdev user,id=n0 -device virtio-net-pci,netdev=n0)

printf 'Starting MIND CORE (aarch64) in QEMU: %s, %s; Ctrl+A X leaves\n' "$QEMU_BIN" "$WHAT"
"$QEMU_BIN" -machine virt,gic-version=3,highmem=off -cpu max -m 512 -smp "${MIND_CPUS:-4}" -serial mon:stdio "${DISPLAY_ARGS[@]}" \
    -drive "if=pflash,format=raw,readonly=on,file=$CODE" -drive "if=pflash,format=raw,file=$VARS" \
    "${DISK[@]}" -device ramfb \
    "${NET[@]}" -device virtio-keyboard-pci -device virtio-tablet-pci "$@"
