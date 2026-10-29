#!/usr/bin/env bash
# Run the initial NRL host profile and build the portable libraries, never an OS image.
set -euo pipefail
case "${1:-}" in ""|--expanded) ;; *) echo "usage: check.sh [--expanded]" >&2; exit 2 ;; esac
cd "$(dirname "${BASH_SOURCE[0]}")/../.."
cargo test --manifest-path nrl/Cargo.toml --locked
cargo build --manifest-path nrl/Cargo.toml --locked --release -p nrl-repr --target x86_64-unknown-none
cargo build --manifest-path nrl/Cargo.toml --locked --release -p nrl-repr --target aarch64-unknown-none-softfloat
cargo build --manifest-path nrl/Cargo.toml --locked --release -p nrl-runtime --target x86_64-unknown-none
cargo build --manifest-path nrl/Cargo.toml --locked --release -p nrl-runtime --target aarch64-unknown-none-softfloat
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


cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- pack-model nrl/tests/fixtures/model-v0/plain.json "$nrl_check_dir/regression-input"
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- train-regression "$nrl_check_dir/regression-input" nrl/tests/fixtures/training-v0/data.json nrl/tests/fixtures/training-v0/run.json "$nrl_check_dir/regression"
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- check-experiment "$nrl_check_dir/regression"
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- replay-regression "$nrl_check_dir/regression" "$nrl_check_dir/regression-replay"


cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- import-decoder nrl/reference/tinyllamas-260k/stories260K.bin nrl/reference/tinyllamas-260k/tok512.bin nrl/reference/tinyllamas-260k/decoder-source.json "$nrl_check_dir/decoder"
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- check-decoder "$nrl_check_dir/decoder"
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- probe-decoder "$nrl_check_dir/decoder" 'Once upon a time' "$nrl_check_dir/decoder-output.json"


cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- calibrate-decoder "$nrl_check_dir/decoder" nrl/tests/fixtures/local-v0/corpus.json 0 64 "$nrl_check_dir/calibration.json"
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- train-local "$nrl_check_dir/decoder" "$nrl_check_dir/calibration.json" nrl/tests/fixtures/local-v0/config.json "$nrl_check_dir/local-partial.json" 2
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- resume-local "$nrl_check_dir/local-partial.json" "$nrl_check_dir/local-complete.json" 8
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- export-local "$nrl_check_dir/decoder" "$nrl_check_dir/local-complete.json" "$nrl_check_dir/local-selected"

cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- benchmark-regression "$nrl_check_dir/regression-input" "$nrl_check_dir/regression-input" nrl/tests/fixtures/training-v0/data.json test nrl/tests/fixtures/study-v0/measurement.json "$nrl_check_dir/regression-base-evaluation.json"
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- benchmark-regression "$nrl_check_dir/regression-input" "$nrl_check_dir/regression/selected-model" nrl/tests/fixtures/training-v0/data.json test nrl/tests/fixtures/study-v0/measurement.json "$nrl_check_dir/regression-selected-evaluation.json"
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- compare-reports "$nrl_check_dir/regression-base-evaluation.json" "$nrl_check_dir/regression-selected-evaluation.json" "$nrl_check_dir/regression-comparison.json"
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- benchmark-language "$nrl_check_dir/decoder" "$nrl_check_dir/decoder" nrl/tests/fixtures/local-v0/corpus.json validation nrl/tests/fixtures/study-v0/measurement.json "$nrl_check_dir/language-base-evaluation.json"
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- benchmark-language "$nrl_check_dir/decoder" "$nrl_check_dir/local-selected" nrl/tests/fixtures/local-v0/corpus.json validation nrl/tests/fixtures/study-v0/measurement.json "$nrl_check_dir/language-selected-evaluation.json"
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- compare-reports "$nrl_check_dir/language-base-evaluation.json" "$nrl_check_dir/language-selected-evaluation.json" "$nrl_check_dir/language-comparison.json"

cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- apply-plan "$nrl_check_dir/decoder" nrl/tests/fixtures/local-v0/corpus.json nrl/tests/fixtures/study-v0/profile.json mixed-linear-pruned "$nrl_check_dir/manual-plan"
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- search-plans "$nrl_check_dir/decoder" nrl/tests/fixtures/local-v0/corpus.json nrl/tests/fixtures/study-v0/profile.json "$nrl_check_dir/plan-search"

cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- recover-plan "$nrl_check_dir/decoder" "$nrl_check_dir/manual-plan/model" nrl/tests/fixtures/local-v0/corpus.json nrl/tests/fixtures/study-v0/profile.json "$nrl_check_dir/recovery"
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- check-recovery "$nrl_check_dir/decoder" "$nrl_check_dir/recovery"
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- replay-recovery "$nrl_check_dir/decoder" "$nrl_check_dir/recovery" "$nrl_check_dir/recovery-replay"

cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- run-budgeted-model "$nrl_check_dir/model-base" nrl/tests/fixtures/model-v0/input.json nrl/tests/fixtures/runtime-v0/policy.json "$nrl_check_dir/runtime-report.json"

cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- profile-decoder "$nrl_check_dir/decoder" nrl/tests/fixtures/local-v0/corpus.json nrl/tests/fixtures/optimization-v0/study.json "$nrl_check_dir/optimization-report.json"

cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- import-embedding nrl/tests/fixtures/embedding-v0/definition.json "$nrl_check_dir/embedding"
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- check-embedding "$nrl_check_dir/embedding"
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- encode-embedding "$nrl_check_dir/embedding" 'moon sky' "$nrl_check_dir/embedding-vector.json"
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- prune-embedding "$nrl_check_dir/embedding" body 2 nrl/tests/fixtures/embedding-v0/replacement-source.json "$nrl_check_dir/embedding-pruned"
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- benchmark-retrieval "$nrl_check_dir/embedding" "$nrl_check_dir/embedding" nrl/tests/fixtures/embedding-v0/data.json test nrl/tests/fixtures/study-v0/measurement.json "$nrl_check_dir/embedding-base-report.json"
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- benchmark-retrieval "$nrl_check_dir/embedding" "$nrl_check_dir/embedding-pruned" nrl/tests/fixtures/embedding-v0/data.json test nrl/tests/fixtures/study-v0/measurement.json "$nrl_check_dir/embedding-pruned-report.json"
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- compare-reports "$nrl_check_dir/embedding-base-report.json" "$nrl_check_dir/embedding-pruned-report.json" "$nrl_check_dir/embedding-comparison.json"

cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- train-adaptation "$nrl_check_dir/decoder" "$nrl_check_dir/manual-plan/model" nrl/tests/fixtures/local-v0/corpus.json nrl/tests/fixtures/adaptation-v0/config.json "$nrl_check_dir/adaptation-partial.json" 1
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- resume-adaptation "$nrl_check_dir/decoder" "$nrl_check_dir/adaptation-partial.json" "$nrl_check_dir/adaptation-complete.json" 8
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- export-adaptation "$nrl_check_dir/decoder" "$nrl_check_dir/adaptation-complete.json" "$nrl_check_dir/adaptation-selected"

cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- profile-generation "$nrl_check_dir/decoder" nrl/tests/fixtures/generation-v0/study.json "$nrl_check_dir/generation-report.json"

cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- train-embedding "$nrl_check_dir/embedding" nrl/tests/fixtures/embedding-v0/data.json nrl/tests/fixtures/embedding-training-v0/config.json "$nrl_check_dir/embedding-training"
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- start-embedding "$nrl_check_dir/embedding" nrl/tests/fixtures/embedding-v0/data.json nrl/tests/fixtures/embedding-training-v0/config.json "$nrl_check_dir/embedding-training-partial.json" 4
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- resume-embedding "$nrl_check_dir/embedding-training-partial.json" "$nrl_check_dir/embedding-training-resumed.json" 16
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- export-trained-embedding "$nrl_check_dir/embedding-training-resumed.json" "$nrl_check_dir/embedding-trained-export"
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- benchmark-trained-retrieval "$nrl_check_dir/embedding" "$nrl_check_dir/embedding-trained-export" nrl/tests/fixtures/embedding-v0/data.json test nrl/tests/fixtures/study-v0/measurement.json "$nrl_check_dir/embedding-trained-benchmark.json"

cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- bind-qsl-decoder "$nrl_check_dir/decoder" nrl/tests/fixtures/qsl-model-v0 nrl/tests/fixtures/qsl-model-v0/binding.json MIT "$nrl_check_dir/qsl-decoder-overlay"
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- check-qsl-decoder "$nrl_check_dir/decoder" "$nrl_check_dir/qsl-decoder-overlay"
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- run-qsl-decoder "$nrl_check_dir/decoder" "$nrl_check_dir/qsl-decoder-overlay" 'Once upon a time' "$nrl_check_dir/qsl-decoder-probe.json"
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- export-qsl-decoder "$nrl_check_dir/decoder" "$nrl_check_dir/qsl-decoder-overlay" "$nrl_check_dir/qsl-decoder-standalone"
cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- probe-decoder "$nrl_check_dir/qsl-decoder-standalone" 'Once upon a time' "$nrl_check_dir/qsl-standalone-probe.json"

if [[ "${1:-}" == "--expanded" ]]; then
    python3 nrl/tools/run_expanded_study.py --executable nrl/target/release/nrl-lab --output "$nrl_check_dir/expanded"
fi
