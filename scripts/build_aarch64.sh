#!/bin/bash
# Builds MIND Core for aarch64 (QEMU `virt`, issues 201-203) into aarch64_root/: the UEFI bootloader (BOOTAA64.EFI),
# and every crate of 02_build.sh's USER_CRATES except the x86-only ones (the LEGACY ISA drivers),
# with the licences and the voice model as on the x86 image. The network driver is built without its legacy (port I/O)
# interface. With --fixtures, also the fault-test service in four variants (aarch64_root/fault-<case>.elf) for
# tests/aarch64_smoke.py, the busy fixture (aarch64_root/fixture-busy_app.elf) for the busy and smp suites and the
# updater's stand-in (aarch64_root/fixture-updater.elf) for the updater suite.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="$ROOT/aarch64_root"
TARGET=aarch64-unknown-none-softfloat
X86_ONLY=" ata ps2_kbd audio_gw bcm_wifi " # crate directories: port I/O (docs/legacy.md); bcm_wifi: no aarch64 target has the chip
mapfile -t CRATES < <(sed -n '/^USER_CRATES=(/,/^)/{s/^ *"\([^"]*\)"$/\1/p}' "$ROOT/02_build.sh")
[[ ${#CRATES[@]} -gt 20 ]] || { echo "USER_CRATES not found in 02_build.sh" >&2; exit 1; }
rm -rf "$OUT"; mkdir -p "$OUT/EFI/BOOT" "$OUT/LICENSES" "$OUT/voice"
# The bootloader and every crate build in parallel, a log each (000-KRN-0020).
source "$ROOT/scripts/build_jobs.sh"
JOBS=$(build_jobs)
LOGS="$ROOT/code_handoff/build-aarch64"
ensure_toolchain "$ROOT" || exit 1
echo ">>> Building the bootloader, the kernel, the services and the programs for aarch64, $JOBS at a time (logs: $LOGS/)..."
declare -A SEEN=()
for entry in "${CRATES[@]}"; do
    crate=${entry%%:*}
    [[ "$X86_ONLY" == *" $crate "* ]] && continue
    # Without its aarch64 section a program links as a fixed-address executable the loaders refuse.
    grep -q "aarch64-unknown-none-softfloat" "$ROOT/$crate/.cargo/config.toml" || { echo "$crate/.cargo/config.toml has no [target.aarch64-unknown-none-softfloat] section" >&2; exit 1; }
done
if ! {
    printf 'bootloader\t%s\t%s\n' "$ROOT/bootloader" "cargo build --release --target aarch64-unknown-uefi"
    for entry in "${CRATES[@]}"; do
        crate=${entry%%:*}
        [[ "$X86_ONLY" == *" $crate "* || -n "${SEEN[$crate]:-}" ]] && continue
        SEEN[$crate]=1
        features=""; [[ "$crate" == virtio_net ]] && features=" --no-default-features"
        printf '%s\t%s\t%s\n' "$crate" "$ROOT/$crate" "cargo build --release --target $TARGET$features"
    done
} | run_jobs "$LOGS" "$JOBS"; then
    echo "!!! aarch64 build failed: the failed builds are named above" >&2; exit 1
fi
cp "$ROOT/bootloader/target/aarch64-unknown-uefi/release/bootloader.efi" "$OUT/EFI/BOOT/BOOTAA64.EFI"
for entry in "${CRATES[@]}"; do
    crate=${entry%%:*}; rest=${entry#*:}; bin=${rest%%:*}; out=${rest#*:}
    [[ "$X86_ONLY" == *" $crate "* ]] && continue
    cp "$ROOT/$crate/target/$TARGET/release/$bin" "$OUT/$out"
done
cp "$ROOT"/LICENSE-MIT "$ROOT"/LICENSE-APACHE "$ROOT"/THIRD_PARTY.md "$ROOT"/LICENSES/*.txt "$OUT/LICENSES/"
cp "$ROOT"/voice/model.bin "$ROOT"/voice/commands.txt "$OUT/voice/"
if [[ "${1:-}" == --fixtures ]]; then
    for case in kernel_read text_write stack_exec undefined; do
        rustc --edition=2021 --target "$TARGET" --crate-type bin -C opt-level=2 -C panic=abort -C relocation-model=pic \
            -C link-arg=-T"$ROOT/app/linker.ld" -C link-arg=-pie -C link-arg=-zmax-page-size=4096 \
            -C link-arg=--no-dynamic-linker -C link-arg=-znotext --cfg "case=\"$case\"" \
            --check-cfg 'cfg(case, values("kernel_read","text_write","stack_exec","undefined"))' \
            "$ROOT/tests/aarch64_fault.rs" -o "$OUT/fault-$case.elf"
    done
    # The busy fixture of the busy and smp suites (issue 203): a loop that never yields.
    rustc --edition=2021 --target "$TARGET" --crate-type bin -C opt-level=2 -C panic=abort -C relocation-model=pic \
        -C link-arg=-T"$ROOT/app/linker.ld" -C link-arg=-pie -C link-arg=-zmax-page-size=4096 \
        -C link-arg=--no-dynamic-linker -C link-arg=-znotext "$ROOT/tests/busy_app.rs" -o "$OUT/fixture-busy_app.elf"
    # The updater's stand-in for the updater suite (351-KRN-0022).
    (cd "$ROOT/tests/updater_stub" && cargo build --release --target "$TARGET")
    cp "$ROOT/tests/updater_stub/target/$TARGET/release/updater_stub" "$OUT/fixture-updater.elf"
fi
# The bootloader signed for Secure Boot if $MIND_SECURE_BOOT_KEYS names the keys (351-UPD-0012), then the boot
# manifest and its signature over everything staged (350-UPD-0002).
if [[ -n "${MIND_SECURE_BOOT_KEYS:-}" ]]; then
    python3 "$ROOT/scripts/secure_boot.py" sign "$OUT/EFI/BOOT/BOOTAA64.EFI" "$MIND_SECURE_BOOT_KEYS" "$OUT/EFI/BOOT/BOOTAA64.EFI"
fi
python3 "$ROOT/scripts/sign_manifest.py" "$OUT"
echo ">>> aarch64 build ready: $OUT"
