#!/usr/bin/env bash
# Run the initial NRL host profile and build the portable libraries, never an OS image.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."
cargo test --manifest-path nrl/Cargo.toml --locked
cargo build --manifest-path nrl/Cargo.toml --locked --release -p nrl-repr --target x86_64-unknown-none
cargo build --manifest-path nrl/Cargo.toml --locked --release -p nrl-repr --target aarch64-unknown-none-softfloat
nrl_check_dir=$(mktemp -d "${TMPDIR:-/tmp}/nrl-check.XXXXXX")
trap 'rm -rf -- "$nrl_check_dir"' EXIT
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- demo nrl/tests/fixtures/f64-v5.json "$nrl_check_dir/demo"

cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- import-qsl nrl/tests/fixtures/qsl-overlay-v1 "$nrl_check_dir/qsl" "MIT; deterministic synthetic fixture"
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- check-block-set "$nrl_check_dir/qsl"
