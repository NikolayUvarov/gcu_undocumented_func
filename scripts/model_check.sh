#!/bin/bash
# Model-checks the assurance models of docs/assurance with TLC (issue 167): the kernel's capability tree, revoke and
# MOVE (CapRevokeMove), and the completion point of revoke on several CPUs (RevokeFlush, for x86 and aarch64); then
# checks that the model of the kernel before issue 167 still shows its counterexample, so the models keep their teeth.
# TLC (tla2tools.jar 1.8.0, MIT) is fetched once into $TLA_TOOLS_DIR (default ~/.cache/mind-tla) and its SHA-256
# checked. Needs Java 11 or later. Usage: scripts/model_check.sh [--quick] (--quick: without the large CapRevokeMove run).
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MODELS="$ROOT/docs/assurance"
DIR="${TLA_TOOLS_DIR:-$HOME/.cache/mind-tla}"
JAR="$DIR/tla2tools-1.8.0.jar"
SHA256=7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d
QUICK=0; [[ "${1:-}" == "--quick" ]] && QUICK=1

mkdir -p "$DIR"
if ! echo "$SHA256  $JAR" | sha256sum -c --status 2>/dev/null; then
    curl -sSfL -o "$JAR.part" https://github.com/tlaplus/tlaplus/releases/download/v1.8.0/tla2tools.jar
    echo "$SHA256  $JAR.part" | sha256sum -c --status || { echo "tla2tools.jar: unexpected SHA-256" >&2; rm -f "$JAR.part"; exit 2; }
    mv "$JAR.part" "$JAR"
fi
WORK="$(mktemp -d)"; trap 'rm -rf "$WORK"' EXIT

# One TLC run in a scratch directory (its state files and traces stay out of the tree); prints TLC's verdict.
tlc() {
    local spec=$1 config=$2
    (cd "$WORK" && java -XX:+UseParallelGC -cp "$JAR" tlc2.TLC -workers auto -metadir "$WORK/states" -cleanup -noGenerateSpecTE \
        -config "$MODELS/$config" "$MODELS/$spec.tla" 2>&1 | grep -v '^Picked up') > "$WORK/out" || true
    grep -E "violated|No error has been found|TLC threw|Error: (Parsing|Deadlock)" "$WORK/out" | head -2
    grep "distinct states found" "$WORK/out" | tail -1
}

failed=0
check() { # spec config expect(pass|fail) label
    echo "== $4"
    local out; out=$(tlc "$1" "$2")
    echo "$out"
    if [[ $3 == pass ]] && ! grep -q "No error has been found" <<<"$out"; then failed=1; fi
    if [[ $3 == fail ]] && ! grep -q "Invariant OneWriter is violated" <<<"$out"; then failed=1; fi
}

if [[ $QUICK == 0 ]]; then check CapRevokeMove CapRevokeMove.cfg pass "capabilities, revoke and MOVE (the kernel)"; fi
check RevokeFlush RevokeFlush.cfg pass "revoke's completion point, x86 (flush on the next CR3 load)"
check RevokeFlush RevokeFlush_aarch64.cfg pass "revoke's completion point, aarch64 (broadcast invalidation)"
check CapRevokeMove CapRevokeMove_before167.cfg fail "the kernel before issue 167: two writers expected"
[[ $failed == 0 ]] && echo "MODELS: PASS" || { echo "MODELS: FAIL"; exit 1; }
