#!/bin/bash
set -e

echo ">>> Checking the environment..."
if ! command -v rustup &> /dev/null; then
    echo "Rust not found. Installing..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
else
    echo "Rust is installed."
fi

echo ">>> Installing the pinned toolchain from rust-toolchain.toml..."
cd "$(dirname "${BASH_SOURCE[0]}")"
rustup toolchain install   # reads rust-toolchain.toml (rustup 1.28+)

echo ">>> Environment is ready to build!"
