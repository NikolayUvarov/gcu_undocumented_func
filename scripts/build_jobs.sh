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
