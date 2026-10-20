#!/usr/bin/env bash
# Builds the multi-device's firmware for every board kind into dist/, and runs its host tests (800-LNK-0005).
# Usage: devices/rp2-link/build.sh [board ...]   (default: every board; boards: rp2040-zero rp2040-yd rp2040-pico
# rp2350-zero rp2350-pico2). The toolchain: arm-none-eabi-gcc, cmake, ninja and the Pico SDK 2.2, found through
# $PICO_SDK_PATH or the maintainer's /opt/rp2040sens01-toolchain/env.sh (its pico-sdk-2.2.0).
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BOARDS=("$@")
[ ${#BOARDS[@]} -gt 0 ] || BOARDS=(rp2040-zero rp2040-yd rp2040-pico rp2350-zero rp2350-pico2)

cc -std=c11 -Wall -Wextra -Werror -O1 -o "${TMPDIR:-/tmp}/rp2-link-host-test" "$HERE/tests/host_test.c" "$HERE/src/bridge.c" "$HERE/src/control.c" "$HERE/src/led.c"
"${TMPDIR:-/tmp}/rp2-link-host-test"

if [ -z "${PICO_SDK_PATH:-}" ] && [ -f /opt/rp2040sens01-toolchain/env.sh ]; then . /opt/rp2040sens01-toolchain/env.sh; fi
# The status LED library (pico_status_led) came with SDK 2.2: take that version beside an older active one.
if [ -f "${PICO_SDK_PATH:-}/pico_sdk_init.cmake" ] && [ ! -d "$PICO_SDK_PATH/src/rp2_common/pico_status_led" ] && [ -d "$PICO_SDK_PATH-2.2.0" ]; then
    export PICO_SDK_PATH="$PICO_SDK_PATH-2.2.0"
fi
[ -n "${PICO_SDK_PATH:-}" ] || { echo "build.sh: set PICO_SDK_PATH to a Pico SDK 2.2" >&2; exit 2; }
mkdir -p "$HERE/dist" "$HERE/build"
for board in "${BOARDS[@]}"; do
    out="$HERE/build/$board"
    cmake -S "$HERE" -B "$out" -G Ninja -DLINK_BOARD="$board" -DCMAKE_BUILD_TYPE=Release > "$out.log" 2>&1 || { cat "$out.log"; exit 1; }
    ninja -C "$out" >> "$out.log" 2>&1 || { tail -40 "$out.log"; exit 1; }
    cp "$out/rp2_link.uf2" "$HERE/dist/mind-link-$board.uf2"
    echo "dist/mind-link-$board.uf2: $(stat -c %s "$HERE/dist/mind-link-$board.uf2") bytes, SHA-256 $(sha256sum "$HERE/dist/mind-link-$board.uf2" | cut -c1-16)…"
done
