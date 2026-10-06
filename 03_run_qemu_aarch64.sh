#!/usr/bin/env bash
# Runs MIND CORE for aarch64 on QEMU's `virt` machine with UEFI firmware (issues 201-205): the shell on the PL011 in
# this terminal, the screen (ramfb) in a window where there is a desktop session, a VirtIO network card, keyboard and
# tablet; the ECAM below 4 GiB (highmem=off); four CPUs.
#   ./03_run_qemu_aarch64.sh                 the build in aarch64_root/ (ARCH=aarch64 ./02_build.sh) as a FAT disk
#   ./03_run_qemu_aarch64.sh --image [file]  the USB image (./04_make_usb_image_aarch64.sh), as a USB stick on xHCI,
#                                            the way a board finds it; QEMU writes nothing to the file
# MIND_CPUS=n, MIND_MEMORY=size (QEMU -m, default 512M; above 3G the machine places RAM and PCI above 4 GiB),
# MIND_NET=none, MIND_DISPLAY=none, MIND_AAVMF_CODE / MIND_AAVMF_VARS (firmware), QEMU=<binary>; further arguments go
# to QEMU. On macOS with Apple Silicon it runs with the hypervisor (HVF) at the host CPU's speed (Homebrew's qemu).
# Leave QEMU with Ctrl+A X.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

fail() { printf 'ERROR: %s\n' "$*" >&2; exit 1; }

QEMU_BIN="${QEMU:-qemu-system-aarch64}"
command -v -- "$QEMU_BIN" >/dev/null 2>&1 || fail "QEMU for aarch64 not found. Debian/Ubuntu/WSL: sudo apt install qemu-system-arm qemu-efi-aarch64; Fedora: qemu-system-aarch64 edk2-aarch64; macOS: brew install qemu; or set QEMU=/path/to/qemu-system-aarch64."
# On an Apple Silicon Mac the hypervisor runs the guest on the host CPU; elsewhere QEMU emulates one (TCG).
ACCEL=(-cpu max)
[[ "$(uname -s)" == Darwin && "$(uname -m)" == arm64 ]] && ACCEL=(-accel hvf -cpu host)
SHARE="$(cd "$(dirname "$(command -v -- "$QEMU_BIN")")/.." && pwd)/share/qemu" # where QEMU keeps its firmware (Homebrew)

# Firmware: AAVMF (Debian, Ubuntu), edk2 (Fedora), or the one QEMU ships; the variables are a private copy.
CODE="${MIND_AAVMF_CODE:-}"; VARS_TEMPLATE="${MIND_AAVMF_VARS:-}"
if [[ -z "$CODE" ]]; then
    for pair in /usr/share/AAVMF/AAVMF_CODE.fd:/usr/share/AAVMF/AAVMF_VARS.fd \
                /usr/share/edk2/aarch64/QEMU_EFI-pflash.raw:/usr/share/edk2/aarch64/vars-template-pflash.raw \
                /usr/share/qemu/edk2-aarch64-code.fd:/usr/share/qemu/edk2-arm-vars.fd \
                "$SHARE/edk2-aarch64-code.fd:$SHARE/edk2-arm-vars.fd"; do
        if [[ -f "${pair%%:*}" && -f "${pair##*:}" ]]; then CODE="${pair%%:*}"; VARS_TEMPLATE="${pair##*:}"; break; fi
    done
fi
[[ -n "$CODE" && -f "$CODE" && -f "$VARS_TEMPLATE" ]] || fail "No aarch64 UEFI firmware: install qemu-efi-aarch64 (Debian/Ubuntu), edk2-aarch64 (Fedora) or qemu (Homebrew), or set MIND_AAVMF_CODE and MIND_AAVMF_VARS."
VARS="$(mktemp "${TMPDIR:-/tmp}/mind-aavmf-vars.XXXXXX")"; trap 'rm -f "$VARS"' EXIT; cp "$VARS_TEMPLATE" "$VARS"
# The variable store must be as large as the code (64 MiB for QEMU's own firmware): the template is padded.
CODE_BYTES=$(wc -c < "$CODE"); VARS_BYTES=$(wc -c < "$VARS")
(( VARS_BYTES < CODE_BYTES )) && dd if=/dev/zero bs=1 count=0 seek="$CODE_BYTES" of="$VARS" 2>/dev/null
# RAM: QEMU's virt keeps memory and PCI below 4 GiB with highmem=off (at most 3 GiB of RAM); more needs highmem=on.
MEMORY="${MIND_MEMORY:-512M}"
HIGHMEM=off
case "$MEMORY" in *[Gg]) (( ${MEMORY%[Gg]} > 3 )) && HIGHMEM=on ;; esac

# The disk: the build directory, or the USB image as a USB stick (snapshot: what the system writes stays in QEMU).
if [[ "${1:-}" == --image ]]; then
    shift
    IMAGE="$ROOT/dist/mind-core-usb-aarch64.img"
    if [[ $# -gt 0 && "$1" != -* ]]; then IMAGE="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"; shift; fi
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

printf 'Starting MIND CORE (aarch64) in QEMU: %s, %s, %s of RAM, %s; Ctrl+A X leaves\n' "$QEMU_BIN" "$WHAT" "$MEMORY" "${ACCEL[*]}"
"$QEMU_BIN" -machine "virt,gic-version=3,highmem=$HIGHMEM" "${ACCEL[@]}" -m "$MEMORY" -smp "${MIND_CPUS:-4}" -serial mon:stdio "${DISPLAY_ARGS[@]}" \
    -drive "if=pflash,format=raw,readonly=on,file=$CODE" -drive "if=pflash,format=raw,file=$VARS" \
    "${DISK[@]}" -device ramfb \
    "${NET[@]}" -device virtio-keyboard-pci -device virtio-tablet-pci "$@"
