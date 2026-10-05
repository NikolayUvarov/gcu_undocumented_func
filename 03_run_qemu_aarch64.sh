#!/bin/bash
# Runs the aarch64 build (scripts/build_aarch64.sh) on QEMU's `virt` machine with AAVMF (issue 201). The console is
# the PL011 on this terminal; the services that need devices are not there yet (issue 202). Extra arguments go to QEMU.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
[[ -f "$ROOT/aarch64_root/kernel.elf" ]] || { echo "Missing aarch64_root/kernel.elf. Run ./scripts/build_aarch64.sh first." >&2; exit 1; }
CODE="${MIND_AAVMF_CODE:-/usr/share/AAVMF/AAVMF_CODE.fd}"; VARS_TEMPLATE="${MIND_AAVMF_VARS:-/usr/share/AAVMF/AAVMF_VARS.fd}"
[[ -f "$CODE" && -f "$VARS_TEMPLATE" ]] || { echo "AAVMF firmware not found (Debian/Ubuntu: qemu-efi-aarch64)." >&2; exit 1; }
VARS="$(mktemp)"; trap 'rm -f "$VARS"' EXIT; cp "$VARS_TEMPLATE" "$VARS"
qemu-system-aarch64 -machine virt,gic-version=3 -cpu max -m 512 -nographic \
    -drive "if=pflash,format=raw,readonly=on,file=$CODE" -drive "if=pflash,format=raw,file=$VARS" \
    -drive "format=raw,file=fat:rw:${ROOT//,/,,}/aarch64_root" -device ramfb "$@"
