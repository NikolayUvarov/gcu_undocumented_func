#!/bin/bash
# The kernel as a module a page drives (215-PRT-0017): built for wasm64-unknown-unknown with build-std, so that usize and
# pointers are the aarch64 programs' 64 bits, then lowered to wasm32 by Binaryen's wasm-opt (web/package.json pins it),
# which every browser and Node 22 run. Writes web/build/kernel.wasm, and with the aarch64 build (aarch64_root/) the boot
# volume web/build/volume.img: the ISO's (scripts/make_iso.py), with the tab's wm settings. WASM_OPT names another wasm-opt.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="$ROOT/web/build"
TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/kernel/target}"
WASM_OPT="${WASM_OPT:-$ROOT/web/node_modules/.bin/wasm-opt}"
if [[ ! -x "$WASM_OPT" ]]; then
    echo ">>> Fetching Binaryen as web/package-lock.json pins it..."
    (cd "$ROOT/web" && npm ci --no-audit --no-fund)
fi
mkdir -p "$OUT"
(cd "$ROOT/kernel" && cargo build --release --target wasm64-unknown-unknown -Z build-std=core,alloc --target-dir "$TARGET_DIR")
"$WASM_OPT" --enable-memory64 --enable-bulk-memory --enable-sign-ext --enable-mutable-globals --enable-nontrapping-float-to-int \
    --enable-multivalue --enable-reference-types --memory64-lowering --table64-lowering -O2 \
    "$TARGET_DIR/wasm64-unknown-unknown/release/kernel.wasm" -o "$OUT/kernel.wasm"
echo ">>> $OUT/kernel.wasm: $(stat -c %s "$OUT/kernel.wasm") bytes"
if [[ -d "$ROOT/aarch64_root" ]]; then
    WORK="$(mktemp -d)"; trap 'rm -rf "$WORK"' EXIT
    python3 "$ROOT/scripts/make_iso.py" --arch aarch64 --output "$WORK/mind.iso" --force >/dev/null
    # The ISO's boot volume is its MBR's one partition (make_iso.py).
    python3 -c 'import struct, sys; iso = open(sys.argv[1], "rb").read(); start, count = struct.unpack_from("<II", iso, 454); open(sys.argv[2], "wb").write(iso[start * 512:(start + count) * 512])' "$WORK/mind.iso" "$OUT/volume.img"
    mcopy -o -i "$OUT/volume.img" "$ROOT/web/wm.conf" ::/data/wm.conf
    echo ">>> $OUT/volume.img: $(stat -c %s "$OUT/volume.img") bytes"
else
    echo ">>> no aarch64_root/: build it (ARCH=aarch64 ./02_build.sh) for the boot volume"
fi
