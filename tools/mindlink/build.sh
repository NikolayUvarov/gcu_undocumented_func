#!/usr/bin/env bash
# Builds mindlink for Linux and, when the toolchain has the target, for Windows (800-LNK-0004, 0007).
#   target/release/mindlink                              Linux and WSL
#   target/x86_64-pc-windows-gnu/release/mindlink.exe    Windows 10 and later, no DLL beyond Windows' own
# The Windows build needs `rustup target add x86_64-pc-windows-gnu` and MinGW-w64's linker (x86_64-w64-mingw32-gcc).
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"
cargo build --release
echo "target/release/mindlink"
if rustup target list --installed 2>/dev/null | grep -qx x86_64-pc-windows-gnu && command -v x86_64-w64-mingw32-gcc >/dev/null; then
    cargo build --release --target x86_64-pc-windows-gnu
    echo "target/x86_64-pc-windows-gnu/release/mindlink.exe"
else
    echo "no Windows build: rustup target add x86_64-pc-windows-gnu, and install MinGW-w64 (gcc-mingw-w64-x86-64)" >&2
fi
