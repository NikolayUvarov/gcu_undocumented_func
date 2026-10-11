#!/usr/bin/env bash
# The x86 test programs and kernel variants of the QEMU suites, for CI and scripts/ci_local.sh: each build fails the run
# (175-KRN-0046). Cargo rebuilds a variant whose sources changed; a failed build stops the run before stale outputs are tested.
# MIND_TEST_OUT: where the test programs go (default /tmp, where the QEMU suites look).
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
OUT=${MIND_TEST_OUT:-/tmp}
for fixture in busy_app isolation_app heap_app block_app; do
    rustc --edition=2021 --target x86_64-unknown-none --crate-type bin -C opt-level=3 -C panic=abort \
        -C relocation-model=pic -Z relax-elf-relocations=yes -C link-arg=-Tapp/linker.ld \
        "tests/$fixture.rs" -o "$OUT/mind-core-$fixture.elf"
done
(cd tests/updater_stub && cargo build --release --target-dir /tmp/mind-updater-target)
for variant in panic:panic abi:abi loader-abi:loader-abi trial:trial x2apic:x2apic bar-move:bar xsave-pad:xsave-pad protection:protection; do
    (cd kernel && cargo build --release --features "${variant%%:*}-test" --target-dir "/tmp/mind-${variant#*:}-target")
done
