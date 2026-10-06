#!/bin/bash
# Runs the CI's groups (.github/workflows/ci.yml) on this machine and prints a PASS/FAIL table.
# Usage: scripts/ci_local.sh [WHAT] [--only host,x86,aarch64] [--tap] [--no-merge] [--keep] [--list]
#   WHAT (default: the working tree as it is):
#     --main         origin/main;
#     --ref BRANCH   origin/BRANCH merged with origin/main (repeatable);
#     --all          origin/main, then every other origin branch merged with origin/main.
#   Branches are fetched and tested in temporary worktrees ($CI_LOCAL_WORK, default /tmp/mind-ci-work); this tree is not touched.
#   --only  the parts to run (default: all three); --tap  also the netbench group over a tap interface (sudo);
#   --no-merge  test branches as they are; --keep  keep the worktrees; --list  print the groups and exit.
# Logs: $CI_LOCAL_LOGS (default /tmp/mind-ci-local), one file per group, one directory per branch.
# Needs (Ubuntu 24.04): qemu-system-x86 qemu-system-arm qemu-utils ovmf qemu-efi-aarch64 ipxe-qemu dosfstools mtools,
# rustup; the toolchain comes from rust-toolchain.toml. Keep the groups in step with ci.yml.
set -uo pipefail
SELF="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/$(basename "${BASH_SOURCE[0]}")"
ROOT="$(cd "$(dirname "$SELF")/.." && pwd)"
source "$HOME/.cargo/env" 2>/dev/null || true
LOGS="${CI_LOCAL_LOGS:-/tmp/mind-ci-local}"
WORK="${CI_LOCAL_WORK:-/tmp/mind-ci-work}"
ONLY="host,x86,aarch64"
TAP=0
LIST=0
MERGE=1
KEEP=0
TREE=""
REFS=()
ALL=0
while [[ $# -gt 0 ]]; do
    case "$1" in
        --only) ONLY="$2"; shift 2 ;;
        --tap) TAP=1; shift ;;
        --list) LIST=1; shift ;;
        --main) REFS+=(main); shift ;;
        --ref) REFS+=("${2#origin/}"); shift 2 ;;
        --all) ALL=1; shift ;;
        --no-merge) MERGE=0; shift ;;
        --keep) KEEP=1; shift ;;
        --tree) TREE="$2"; shift 2 ;; # internal: run the groups in this tree
        -h|--help) sed -n '2,13p' "$SELF"; exit 0 ;;
        *) echo "unknown option: $1" >&2; exit 2 ;;
    esac
done
want() { [[ ",$ONLY," == *",$1,"* ]]; }

OVMF=/usr/share/ovmf/OVMF.fd
X86="python3 tests/qemu_smoke.py --qemu qemu-system-x86_64 --firmware $OVMF \
 --busy-elf /tmp/mind-core-busy_app.elf --isolation-elf /tmp/mind-core-isolation_app.elf \
 --heap-elf /tmp/mind-core-heap_app.elf --block-elf /tmp/mind-core-block_app.elf \
 --panic-kernel /tmp/mind-panic-target/x86_64-unknown-none/release/kernel"
A64="python3 tests/qemu_smoke.py --arch aarch64"

# Group name | command; a failed "build" step skips the rest of its part.
HOST_GROUPS=(
    "build (x86)|./02_build.sh"
    "network driver without legacy|(cd virtio_net && cargo build --release --no-default-features --target-dir /tmp/virtio-net-modern-only)"
    "host tests|host_tests"
    "models (TLC)|scripts/model_check.sh"
)
X86_GROUPS=(
    "build (x86 test programs)|x86_fixtures"
    "x86: boot, display, network, TLS, shell, memory, clock|$X86 --suites boot,display,net,tls,normal,memory,dzen"
    "x86: services, storage, audio|$X86 --suites services,ahci,audio,tts,listen"
    "x86: scheduling, isolation, heap|$X86 --suites busy,smp,isolation,heap"
    "x86: keys, shell, tools|$X86 --suites keys,shell,tools,windows,wm,tablet,usb"
    "x86: files and block writes|$X86 --suites vfs,edit,disk,block"
    "x86: NVMe boot disk|$X86 --disk nvme --suites vfs"
    "x86: one CPU|$X86 --cpus 1 --suites smp,isolation,heap,services"
    "x86: AVX state|$X86 --cpu-model max --suites busy,smp"
    "x86: AVX state, one CPU|$X86 --cpu-model max --cpus 1 --suites busy,smp"
    "x86: USB image|python3 scripts/make_usb_image.py --no-build --force && python3 tests/usb_image_smoke.py --firmware $OVMF"
)
[[ $TAP == 1 ]] && X86_GROUPS+=("x86: network benchmark (tap)|tap_bench")
A64_GROUPS=(
    "build (aarch64)|ARCH=aarch64 ./02_build.sh --fixtures"
    "aarch64: boot and fault containment|python3 tests/aarch64_smoke.py"
    "aarch64: programs, shell and four CPUs|$A64 --suites normal,shell,smp,busy,usb"
    "aarch64: files, network and TLS|$A64 --suites vfs,net,tls"
    "aarch64: RAM, ACPI and PCI above 4 GiB|$A64 --suites normal,net --machine virt,gic-version=3,highmem=on --memory 6G"
    "aarch64: GICv2 with GICv2m|$A64 --suites normal,smp,net --machine virt,gic-version=2,highmem=off"
    "aarch64: NVMe boot disk|$A64 --suites vfs --disk nvme"
)

host_tests() {
    local t
    rustc --edition=2021 --test tests/runtime.rs -o /tmp/runtime-tests && /tmp/runtime-tests || return 1
    rustc --edition=2021 --test tests/tts_host.rs -o /tmp/tts-tests && /tmp/tts-tests || return 1
    for t in heap keys tui viewer idl rtc sysmon monitor fm block fat edit logd search bmp netring window wm clock virtio_input hid aml gpio pins video line beep console say jpeg script; do
        rustc --edition=2021 --test "tests/${t}_host.rs" -o "/tmp/$t-tests" && "/tmp/$t-tests" || return 1
    done
    rustc --edition=2021 -O --test tests/voice_host.rs -o /tmp/voice-tests && /tmp/voice-tests || return 1
    python3 tests/idl_test.py && python3 tests/font_test.py
}
x86_fixtures() {
    local f
    for f in busy_app isolation_app heap_app block_app; do
        rustc --edition=2021 --target x86_64-unknown-none --crate-type bin -C opt-level=3 -C panic=abort \
            -C relocation-model=pic -Z relax-elf-relocations=yes -C link-arg=-Tapp/linker.ld \
            "tests/$f.rs" -o "/tmp/mind-core-$f.elf" || return 1
    done
    (cd kernel && cargo build --release --features panic-test --target-dir /tmp/mind-panic-target)
}
tap_bench() {
    ip link show mindtap0 >/dev/null 2>&1 || {
        sudo ip tuntap add dev mindtap0 mode tap user "$(id -un)" && sudo ip addr add 10.0.2.2/24 dev mindtap0 &&
            sudo ip link set mindtap0 up
    } || return 1
    $X86 --suites netbench && python3 tests/qemu_smoke.py --qemu qemu-system-x86_64 --firmware "$OVMF" \
        --suites netbench --tap mindtap0 --bench-runs 2
}

if [[ $LIST == 1 ]]; then
    want host && printf '%s\n' "${HOST_GROUPS[@]%%|*}"
    want x86 && printf '%s\n' "${X86_GROUPS[@]%%|*}"
    want aarch64 && printf '%s\n' "${A64_GROUPS[@]%%|*}"
    exit 0
fi

cd "${TREE:-$ROOT}" || exit 2
# Missing tools are reported up front, not as a failure in the middle of the run.
missing=()
for tool in cargo rustup python3 mcopy mkfs.fat qemu-img java; do command -v "$tool" >/dev/null || missing+=("$tool"); done
{ want x86 || want host; } && { command -v qemu-system-x86_64 >/dev/null || missing+=(qemu-system-x86_64); [[ -f $OVMF ]] || missing+=("$OVMF"); }
want aarch64 && { command -v qemu-system-aarch64 >/dev/null || missing+=(qemu-system-aarch64); [[ -f /usr/share/AAVMF/AAVMF_CODE.fd ]] || missing+=(AAVMF); }
if [[ ${#missing[@]} -gt 0 ]]; then
    echo "Missing: ${missing[*]}" >&2
    echo "sudo apt-get install -y --no-install-recommends qemu-system-x86 qemu-system-arm qemu-utils ovmf qemu-efi-aarch64 ipxe-qemu dosfstools mtools default-jre-headless" >&2
    exit 2
fi
rustup toolchain install >/dev/null || exit 2

# Branches: each in its own worktree, merged with main unless --no-merge, then this script with --tree.
safe() { tr -c 'A-Za-z0-9._\n-' '_' <<<"$1"; }
if [[ ${#REFS[@]} -gt 0 || $ALL == 1 ]]; then
    exec 9>"${TMPDIR:-/tmp}/mind-ci-local.lock"
    flock -n 9 || { echo "Another ci_local.sh run is in progress." >&2; exit 3; }
    git fetch origin --prune --quiet || { echo "git fetch failed" >&2; exit 2; }
    if [[ $ALL == 1 ]]; then
        REFS=(main)
        while read -r ref; do [[ $ref != main && $ref != HEAD ]] && REFS+=("$ref"); done \
            < <(git for-each-ref --sort=-committerdate --format='%(refname:lstrip=3)' refs/remotes/origin)
    fi
    rm -rf "$LOGS"; mkdir -p "$LOGS" "$WORK"
    SUMMARY=(); FAILED=0
    opts=(--only "$ONLY"); [[ $TAP == 1 ]] && opts+=(--tap)
    for ref in "${REFS[@]}"; do
        name=$(safe "$ref"); wt="$WORK/$name"
        if ! git rev-parse -q --verify "origin/$ref^{commit}" >/dev/null; then
            SUMMARY+=("FAIL  $ref (no origin/$ref)"); FAILED=1; continue
        fi
        git worktree remove --force "$wt" 2>/dev/null; rm -rf "$wt"; git worktree prune
        git worktree add --detach --quiet "$wt" "origin/$ref" || { SUMMARY+=("FAIL  $ref (worktree)"); FAILED=1; continue; }
        what="$ref @ $(git -C "$wt" rev-parse --short HEAD)"
        if [[ $ref != main && $MERGE == 1 ]]; then
            what+=" + main @ $(git rev-parse --short origin/main)"
            if ! git -C "$wt" -c user.name=ci-local -c user.email=ci-local@localhost merge --no-edit origin/main \
                >"$LOGS/$name-merge.log" 2>&1; then
                SUMMARY+=("FAIL  $what: merge conflict with main ($LOGS/$name-merge.log)"); FAILED=1
                [[ $KEEP == 1 ]] || git worktree remove --force "$wt"
                continue
            fi
        fi
        echo; echo "=== $what"
        # A branch is tested with its own groups when it has this script.
        runner="$SELF"; [[ -x "$wt/scripts/ci_local.sh" ]] && runner="$wt/scripts/ci_local.sh"
        CI_LOCAL_LOGS="$LOGS/$name" "$runner" --tree "$wt" "${opts[@]}"; status=$?
        if [[ $status == 0 ]]; then SUMMARY+=("PASS  $what"); else SUMMARY+=("FAIL  $what (logs: $LOGS/$name)"); FAILED=1; fi
        [[ $KEEP == 1 ]] || git worktree remove --force "$wt"
    done
    echo; echo "=== Summary"; printf '%s\n' "${SUMMARY[@]}" | tee "$LOGS/summary.txt"
    exit $FAILED
fi

rm -rf "$LOGS"; mkdir -p "$LOGS"
NAMES=(); RESULTS=(); TIMES=()
FAILED=0
run_part() {
    local entry name cmd log start status skip=0 n=${#NAMES[@]}
    for entry in "$@"; do
        name=${entry%%|*}; cmd=${entry#*|}
        n=$((n + 1)); log="$LOGS/$(printf '%02d' $n)-$(tr -c 'A-Za-z0-9\n' '_' <<<"$name").log"
        NAMES+=("$name")
        if [[ $skip == 1 ]]; then RESULTS+=(SKIP); TIMES+=(-); echo "SKIP  $name"; continue; fi
        echo "....  $name"
        rm -f /tmp/mind-core-*.log
        start=$(date +%s)
        eval "$cmd" >"$log" 2>&1; status=$?
        TIMES+=("$(( $(date +%s) - start ))s")
        if [[ $status == 0 ]]; then
            RESULTS+=(PASS); echo "PASS  $name"
        else
            RESULTS+=(FAIL); FAILED=1; echo "FAIL  $name  (log: $log)"
            cp /tmp/mind-core-*.log "$LOGS/" 2>/dev/null
            [[ $name == build* ]] && skip=1
        fi
    done
}
# x86 parts first: the aarch64 build replaces the shared outputs.
want host && run_part "${HOST_GROUPS[@]}"
if want x86; then
    want host || X86_GROUPS=("${HOST_GROUPS[0]}" "${X86_GROUPS[@]}")
    run_part "${X86_GROUPS[@]}"
fi
want aarch64 && run_part "${A64_GROUPS[@]}"

echo
printf '%-6s %-8s %s\n' RESULT TIME GROUP
for i in "${!NAMES[@]}"; do printf '%-6s %-8s %s\n' "${RESULTS[$i]}" "${TIMES[$i]}" "${NAMES[$i]}"; done
echo "Logs: $LOGS"
[[ -n "$(git status --porcelain)" ]] && echo "Note: the working tree is not clean (CI's build fails on that):" && git status --short
exit $FAILED
