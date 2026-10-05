#!/bin/bash
# Builds MIND Core for aarch64 (QEMU `virt`, issue 201): the UEFI bootloader, the kernel and the services that run
# there so far, into aarch64_root/ (BOOTAA64.EFI, kernel.elf, *.elf). With --fixtures, also the fault-test service
# in four variants (aarch64_root/fault-<case>.elf) for tests/aarch64_smoke.py.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="$ROOT/aarch64_root"
SERVICES=(init logd rtc virtio_blk loader sysmon keystore compositor ramdisk vfs_server netstack netpolicy tls windows virtio_input shell)
TARGET=aarch64-unknown-none-softfloat
rm -rf "$OUT"; mkdir -p "$OUT/EFI/BOOT"
(cd "$ROOT/bootloader" && cargo build --release --target aarch64-unknown-uefi)
cp "$ROOT/bootloader/target/aarch64-unknown-uefi/release/bootloader.efi" "$OUT/EFI/BOOT/BOOTAA64.EFI"
(cd "$ROOT/kernel" && cargo build --release --target "$TARGET")
cp "$ROOT/kernel/target/$TARGET/release/kernel" "$OUT/kernel.elf"
for service in "${SERVICES[@]}"; do
    (cd "$ROOT/$service" && cargo build --release --target "$TARGET")
    cp "$ROOT/$service/target/$TARGET/release/$service" "$OUT/$service.elf"
done
# The network driver without its legacy (port I/O) interface: modern VirtIO only.
(cd "$ROOT/virtio_net" && cargo build --release --target "$TARGET" --no-default-features)
cp "$ROOT/virtio_net/target/$TARGET/release/virtio_net" "$OUT/virtio_net.elf"
if [[ "${1:-}" == --fixtures ]]; then
    for case in kernel_read text_write stack_exec undefined; do
        rustc --edition=2021 --target "$TARGET" --crate-type bin -C opt-level=2 -C panic=abort -C relocation-model=pic \
            -C link-arg=-T"$ROOT/app/linker.ld" -C link-arg=-pie -C link-arg=-zmax-page-size=4096 \
            -C link-arg=--no-dynamic-linker -C link-arg=-znotext --cfg "case=\"$case\"" \
            --check-cfg 'cfg(case, values("kernel_read","text_write","stack_exec","undefined"))' \
            "$ROOT/tests/aarch64_fault.rs" -o "$OUT/fault-$case.elf"
    done
fi
echo ">>> aarch64 build ready: $OUT"
