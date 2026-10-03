#!/bin/bash
set -e

echo ">>> Checking the environment..."
if ! command -v rustup &> /dev/null; then
    echo "Rust not found. Installing..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
else
    echo "Rust is installed; updating..."
    rustup update
fi

echo ">>> Setting up the toolchain (nightly) and targets..."
rustup default nightly
rustup target add x86_64-unknown-uefi x86_64-unknown-none
rustup component add llvm-tools-preview

echo ">>> Environment is ready to build!"
