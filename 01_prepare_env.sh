#!/bin/bash
set -e

echo ">>> Проверка окружения..."
if ! command -v rustup &> /dev/null; then
    echo "Rust не найден. Запускаю установку..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
else
    echo "Rust установлен, обновляем..."
    rustup update
fi

echo ">>> Настройка Toolchain (Nightly) и таргетов..."
rustup default nightly
rustup target add x86_64-unknown-uefi x86_64-unknown-none
rustup component add llvm-tools-preview

echo ">>> Окружение готово к сборке!"
