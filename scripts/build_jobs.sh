# Build jobs in parallel (000-KRN-0020), sourced by 02_build.sh and scripts/build_aarch64.sh. Each crate builds in its
# own target directory, so the cargo runs are independent; each writes its own log.
#
# run_jobs LOG_DIR LIMIT: the jobs come on stdin, one a line: a name, a directory and a command, separated by tabs.
# At most LIMIT run at once. A line for each job as it ends; the errors of every failed one at the end, with the path
# of its full log. Returns 1 if any failed.
run_jobs() {
    local dir=$1 limit=$2 name path command running=0
    mkdir -p "$dir"
    rm -f "$dir"/*.log "$dir"/*.failed
    while IFS=$'\t' read -r name path command; do
        (
            start=$SECONDS
            if (cd "$path" && eval "$command") > "$dir/$name.log" 2>&1; then
                echo "    ok      $name ($((SECONDS - start)) s)"
            else
                echo "    FAILED  $name ($((SECONDS - start)) s): $dir/$name.log"
                touch "$dir/$name.failed"
            fi
        ) &
        running=$((running + 1))
        if [ "$running" -ge "$limit" ]; then wait -n || true; running=$((running - 1)); fi
    done
    wait
    local failed=("$dir"/*.failed)
    [ -e "${failed[0]}" ] || return 0
    for marker in "${failed[@]}"; do
        name=$(basename "$marker" .failed)
        echo
        echo "=== $name failed; its full log: $dir/$name.log ==="
        if grep -qE "^error(\[|:)" "$dir/$name.log"; then grep -E -A12 "^error(\[|:)" "$dir/$name.log" | head -80; else tail -30 "$dir/$name.log"; fi
    done
    return 1
}

# The number of jobs at once: $MIND_BUILD_JOBS, or one per processor.
build_jobs() { echo "${MIND_BUILD_JOBS:-$(nproc 2>/dev/null || getconf _NPROCESSORS_ONLN 2>/dev/null || echo 4)}"; }

# ensure_toolchain ROOT: the pinned toolchain of ROOT/rust-toolchain.toml, with its components and targets, installed once
# before the jobs; in parallel each cargo would ask rustup for a missing part, and the downloads race on one file
# (000-KRN-0060). Nothing is fetched when everything is there. Returns 1 with a message if rustup cannot install it.
ensure_toolchain() {
    local file="$1/rust-toolchain.toml" channel have item components targets missing=() add=()
    [ -f "$file" ] || return 0
    channel=$(sed -n 's/^channel *= *"\([^"]*\)".*/\1/p' "$file")
    [ -n "$channel" ] || return 0
    components=$(sed -n 's/^components *= *\[\(.*\)\].*/\1/p' "$file" | tr -d '",')
    targets=$(sed -n 's/^targets *= *\[\(.*\)\].*/\1/p' "$file" | tr -d '",')
    have=$(rustup component list --installed --toolchain "$channel" 2>/dev/null) || have=""
    for item in $components; do grep -qE "^$item(-|\$)" <<<"$have" || { missing+=("$item"); add+=(--component "$item"); }; done
    for item in $targets; do grep -qx "rust-std-$item" <<<"$have" || { missing+=("$item"); add+=(--target "$item"); }; done
    [ -n "$have" ] && [ ${#missing[@]} -eq 0 ] && return 0
    local what="is not installed"; [ -n "$have" ] && what="lacks ${missing[*]}"
    echo ">>> The pinned toolchain $channel $what: installing it once, before the parallel build..."
    rustup toolchain install "$channel" --profile minimal "${add[@]}" || { echo "!!! rustup could not install the pinned toolchain $channel (rust-toolchain.toml)" >&2; return 1; }
}
