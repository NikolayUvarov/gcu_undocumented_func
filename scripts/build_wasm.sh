#!/bin/bash
# The kernel as a module a page drives (215-PRT-0017): built for wasm64-unknown-unknown with build-std, so that usize and
# pointers are the aarch64 programs' 64 bits, then lowered to wasm32 by Binaryen's wasm-opt (web/package.json pins it),
# which every browser and Node 22 run. Writes web/build/kernel.wasm. WASM_OPT names another wasm-opt.
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
