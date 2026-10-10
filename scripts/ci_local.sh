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
#   --group TEXT  only the groups whose name contains TEXT (repeatable); a part's build runs only when named, so the
#     groups after it use the tree's earlier build: a long part in steps of its own (docs/effector.md).
# Logs: $CI_LOCAL_LOGS (default /tmp/mind-ci-local), one file per group, one directory per branch.
# Needs (Ubuntu 24.04): qemu-system-x86 qemu-system-arm qemu-utils ovmf qemu-efi-aarch64 ipxe-qemu dosfstools mtools swtpm,
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
PICK=()
while [[ $# -gt 0 ]]; do
    case "$1" in
        --only) ONLY="$2"; shift 2 ;;
        --group) PICK+=("$2"); shift 2 ;;
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
 --heap-elf /tmp/mind-core-heap_app.elf --block-elf /tmp/mind-core-block_app.elf --updater-elf /tmp/mind-updater-target/x86_64-unknown-none/release/updater_stub \
 --panic-kernel /tmp/mind-panic-target/x86_64-unknown-none/release/kernel --abi-kernel /tmp/mind-abi-target/x86_64-unknown-none/release/kernel \
 --loader-abi-kernel /tmp/mind-loader-abi-target/x86_64-unknown-none/release/kernel --trial-kernel /tmp/mind-trial-target/x86_64-unknown-none/release/kernel --bar-kernel /tmp/mind-bar-target/x86_64-unknown-none/release/kernel"
A64="python3 tests/qemu_smoke.py --arch aarch64"

# Group name | command; a failed "build" step skips the rest of its part.
HOST_GROUPS=(
    "build (x86)|./02_build.sh"
    "network driver without legacy|(cd virtio_net && cargo build --release --no-default-features --target-dir /tmp/virtio-net-modern-only)"
    "host tests|scripts/host_tests.sh"
    "models (TLC)|scripts/model_check.sh"
)
X86_GROUPS=(
    "build (x86 test programs)|scripts/x86_fixtures.sh"
    "x86: boot, display, network, TLS, shell, memory, clock|$X86 --suites boot,display,net,tls,normal,memory,dzen"
    "x86: services, storage, audio|$X86 --suites services,ahci,audio,tts,listen,hda"
    "x86: scheduling, isolation, heap|$X86 --suites busy,smp,isolation,heap"
    "x86: keys, shell, tools|$X86 --suites keys,shell,tools,windows,wm,tablet,usb,ehci"
    "x86: files and block writes|$X86 --suites vfs,edit,disk,block,store,storefaults,updater"
    "x86: NVMe boot disk|$X86 --disk nvme --suites vfs"
    "x86: 16 CPUs|$X86 --cpus 16 --suites normal"
    "x86: RAM above 4 GiB|$X86 --memory 6G --suites normal,display,net,vfs"
    "x86: one CPU|$X86 --cpus 1 --suites smp,isolation,heap,services"
    "x86: AVX state|$X86 --cpu-model max --suites busy,smp"
    "x86: AVX state, one CPU|$X86 --cpu-model max --cpus 1 --suites busy,smp"
    "x86: no PIT|$X86 --machine pit=off --suites normal,busy"
    "x86: x2APIC|$X86 --cpu-model max --kernel /tmp/mind-x2apic-target/x86_64-unknown-none/release/kernel --suites normal,busy,smp,isolation"
    "x86: padded vector area|$X86 --cpu-model max --kernel /tmp/mind-xsave-pad-target/x86_64-unknown-none/release/kernel --suites busy,smp,isolation"
    "x86: protection probes|$X86 --cpu-model max --kernel /tmp/mind-protection-target/x86_64-unknown-none/release/kernel --suites isolation"
    "x86: USB image|python3 scripts/make_usb_image.py --no-build --force && python3 tests/usb_image_smoke.py --firmware $OVMF"
    "x86: Secure Boot with our keys|python3 tests/secure_boot_smoke.py && python3 tests/dbx_update_smoke.py"
    "x86: reproducible build|scripts/reproducible.sh"
)
[[ $TAP == 1 ]] && X86_GROUPS+=("x86: network benchmark (tap)|tap_bench")
A64_GROUPS=(
    "build (aarch64)|ARCH=aarch64 ./02_build.sh --fixtures"
    "aarch64: boot and fault containment|python3 tests/aarch64_smoke.py"
    "aarch64: programs, shell and four CPUs|$A64 --suites normal,shell,smp,busy,usb,devicetree,efivar"
    "aarch64: files, network and TLS|$A64 --suites vfs,store,storefaults,net,tls,updater"
    "aarch64: RAM, ACPI and PCI above 4 GiB|$A64 --suites normal,net --machine virt,gic-version=3,highmem=on --memory 6G"
    "aarch64: GICv2 with GICv2m|$A64 --suites normal,smp,net --machine virt,gic-version=2,highmem=off"
    "aarch64: NVMe boot disk|$A64 --suites vfs --disk nvme"
    "aarch64: 16 CPUs|$A64 --cpus 16 --suites normal"
)

tap_bench() {
    ip link show mindtap0 >/dev/null 2>&1 || {
        sudo ip tuntap add dev mindtap0 mode tap user "$(id -un)" && sudo ip addr add 10.0.2.2/24 dev mindtap0 &&
            sudo ip link set mindtap0 up
    } || return 1
    $X86 --suites netbench && python3 tests/qemu_smoke.py --qemu qemu-system-x86_64 --firmware "$OVMF" \
        --suites netbench --tap mindtap0 --bench-runs 2
}

# With --group, a group runs only if its name contains one of the texts.
selected() { local g; [[ ${#PICK[@]} == 0 ]] && return 0; for g in "${PICK[@]}"; do [[ $1 == *"$g"* ]] && return 0; done; return 1; }
if [[ $LIST == 1 ]]; then
    for entry in $(want host && printf '%s\n' "${HOST_GROUPS[@]%%|*}" | tr ' ' '\001'; want x86 && printf '%s\n' "${X86_GROUPS[@]%%|*}" | tr ' ' '\001';
                   want aarch64 && printf '%s\n' "${A64_GROUPS[@]%%|*}" | tr ' ' '\001'); do
        entry=$(tr '\001' ' ' <<<"$entry"); selected "$entry" && echo "$entry"
    done
    exit 0
fi

cd "${TREE:-$ROOT}" || exit 2
# Missing tools are reported up front, not as a failure in the middle of the run.
missing=()
for tool in cargo rustup python3 mcopy mkfs.fat qemu-img swtpm; do command -v "$tool" >/dev/null || missing+=("$tool"); done
want host && { command -v java >/dev/null || missing+=(java); } # the models (TLC) only
{ want x86 || want host; } && { command -v qemu-system-x86_64 >/dev/null || missing+=(qemu-system-x86_64); [[ -f $OVMF ]] || missing+=("$OVMF"); }
want aarch64 && { command -v qemu-system-aarch64 >/dev/null || missing+=(qemu-system-aarch64); [[ -f /usr/share/AAVMF/AAVMF_CODE.fd ]] || missing+=(AAVMF); }
if [[ ${#missing[@]} -gt 0 ]]; then
    echo "Missing: ${missing[*]}" >&2
    echo "sudo apt-get install -y --no-install-recommends qemu-system-x86 qemu-system-arm qemu-utils ovmf qemu-efi-aarch64 ipxe-qemu dosfstools mtools swtpm swtpm-tools default-jre-headless" >&2
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
    for g in "${PICK[@]}"; do opts+=(--group "$g"); done
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
        selected "$name" || continue
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
