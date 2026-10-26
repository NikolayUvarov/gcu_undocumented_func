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

cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- pack-model nrl/tests/fixtures/model-v0/definition.json "$nrl_check_dir/model-base"
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- run-model "$nrl_check_dir/model-base" nrl/tests/fixtures/model-v0/input.json "$nrl_check_dir/base-output.json"
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- export-overlay "$nrl_check_dir/model-base" nrl/tests/fixtures/model-v0/replacements.json "$nrl_check_dir/model-overlay"
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- run-model "$nrl_check_dir/model-overlay" nrl/tests/fixtures/model-v0/input.json "$nrl_check_dir/overlay-output.json" "$nrl_check_dir/model-base"
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- export-standalone "$nrl_check_dir/model-overlay" "$nrl_check_dir/model-flat" "$nrl_check_dir/model-base"
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- run-model "$nrl_check_dir/model-flat" nrl/tests/fixtures/model-v0/input.json "$nrl_check_dir/flat-output.json"
