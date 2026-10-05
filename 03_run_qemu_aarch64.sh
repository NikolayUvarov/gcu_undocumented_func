#!/bin/bash
# Runs the aarch64 build (scripts/build_aarch64.sh) on QEMU's `virt` machine with AAVMF (issues 201-202): the shell on
# the PL011 in this terminal, the screen (ramfb) in a window, VirtIO disk, network card, keyboard and tablet; the ECAM
# below 4 GiB (highmem=off). Extra arguments go to QEMU.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
[[ -f "$ROOT/aarch64_root/kernel.elf" ]] || { echo "Missing aarch64_root/kernel.elf. Run ./scripts/build_aarch64.sh first." >&2; exit 1; }
CODE="${MIND_AAVMF_CODE:-/usr/share/AAVMF/AAVMF_CODE.fd}"; VARS_TEMPLATE="${MIND_AAVMF_VARS:-/usr/share/AAVMF/AAVMF_VARS.fd}"
[[ -f "$CODE" && -f "$VARS_TEMPLATE" ]] || { echo "AAVMF firmware not found (Debian/Ubuntu: qemu-efi-aarch64)." >&2; exit 1; }
VARS="$(mktemp)"; trap 'rm -f "$VARS"' EXIT; cp "$VARS_TEMPLATE" "$VARS"
# The screen in a window where there is a desktop session, else only the console on this terminal. The network card
# is a -device: QEMU gives a -nic card on virt no MSI-X.
DISPLAY_ARGS=(-display none); [[ -n "${DISPLAY:-}${WAYLAND_DISPLAY:-}" ]] && DISPLAY_ARGS=()
qemu-system-aarch64 -machine virt,gic-version=3,highmem=off -cpu max -m 512 -serial mon:stdio "${DISPLAY_ARGS[@]}" \
    -drive "if=pflash,format=raw,readonly=on,file=$CODE" -drive "if=pflash,format=raw,file=$VARS" \
    -drive "format=raw,file=fat:rw:${ROOT//,/,,}/aarch64_root" -device ramfb \
    -netdev user,id=n0 -device virtio-net-pci,netdev=n0 -device virtio-keyboard-pci -device virtio-tablet-pci "$@"
