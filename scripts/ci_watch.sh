#!/bin/bash
# Watches origin and runs scripts/ci_local.sh on each new state of main and of the other branches (each merged with main).
# Usage: scripts/ci_watch.sh [--interval SECONDS] [--once] [--branches GLOB] [--no-branches] [-- ci_local.sh options]
#   --interval  pause between checks (default 600); --once  one check, then exit;
#   --branches  which origin branches besides main (default '*'); --no-branches  main only.
#   Options after -- go to ci_local.sh (e.g. -- --only aarch64).
# If this checkout is on a clean main, it is fast-forwarded to origin/main, and the watcher restarts when it changed itself.
# State and history: $CI_WATCH_DIR (default ~/.cache/mind-ci-watch): history.log (one line per run), runs/<time>-<branch>/.
# A branch is tested again when its commit or main's changes; a failed run is not repeated until one of them does.
set -uo pipefail
SELF="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/$(basename "${BASH_SOURCE[0]}")"
ROOT="$(cd "$(dirname "$SELF")/.." && pwd)"
DIR="${CI_WATCH_DIR:-$HOME/.cache/mind-ci-watch}"
KEEP_RUNS=30
INTERVAL=600
ONCE=0
GLOB='*'
BRANCHES=1
LOCAL_OPTS=()
ARGS=("$@")
while [[ $# -gt 0 ]]; do
    case "$1" in
        --interval) INTERVAL="$2"; shift 2 ;;
        --once) ONCE=1; shift ;;
        --branches) GLOB="$2"; shift 2 ;;
        --no-branches) BRANCHES=0; shift ;;
        --) shift; LOCAL_OPTS=("$@"); break ;;
        -h|--help) sed -n '2,9p' "$SELF"; exit 0 ;;
        *) echo "unknown option: $1" >&2; exit 2 ;;
    esac
done
cd "$ROOT" || exit 2
mkdir -p "$DIR/state" "$DIR/runs"
exec 8>"$DIR/watch.lock"
flock -n 8 || { echo "Another ci_watch.sh is running." >&2; exit 3; }

log() { echo "$(date '+%F %T') $*"; }
safe() { tr -c 'A-Za-z0-9._\n-' '_' <<<"$1"; }
notify() { command -v notify-send >/dev/null && notify-send "MIND Core CI" "$1" 2>/dev/null; true; }

# Fast-forward a clean main checkout; restart if this script changed.
update_checkout() {
    [[ $(git symbolic-ref --short -q HEAD) == main ]] || return 0
    [[ -z "$(git status --porcelain --untracked-files=no)" ]] || { log "main has local changes: not updated"; return 0; }
    [[ $(git rev-parse HEAD) != $(git rev-parse origin/main) ]] || return 0
    local before; before=$(git hash-object "$SELF")
    git merge --ff-only --quiet origin/main || { log "main cannot be fast-forwarded: not updated"; return 0; }
    log "checkout updated to $(git rev-parse --short HEAD)"
    if [[ $(git hash-object "$SELF") != "$before" ]]; then
        log "ci_watch.sh changed: restarting"
        exec 8>&-
        exec "$SELF" "${ARGS[@]}"
    fi
}

check() {
    local main ref key name stamp logs status result
    git fetch origin --prune --quiet || { log "git fetch failed; next try in ${INTERVAL}s"; return; }
    update_checkout
    main=$(git rev-parse origin/main)
    local refs=(main)
    if [[ $BRANCHES == 1 ]]; then
        while read -r ref; do
            [[ $ref == main || $ref == HEAD ]] && continue
            [[ $ref == $GLOB ]] && refs+=("$ref")
        done < <(git for-each-ref --sort=-committerdate --format='%(refname:lstrip=3)' refs/remotes/origin)
    fi
    for ref in "${refs[@]}"; do
        name=$(safe "$ref")
        key=$(git rev-parse "origin/$ref")
        [[ $ref != main ]] && key+=" $main"
        [[ "$(cat "$DIR/state/$name" 2>/dev/null)" == "$key" ]] && continue
        stamp=$(date '+%Y%m%d-%H%M%S'); logs="$DIR/runs/$stamp-$name"
        log "testing $ref @ ${key:0:7}"
        if [[ $ref == main ]]; then
            CI_LOCAL_LOGS="$logs" "$ROOT/scripts/ci_local.sh" --main "${LOCAL_OPTS[@]}"; status=$?
        else
            CI_LOCAL_LOGS="$logs" "$ROOT/scripts/ci_local.sh" --ref "$ref" "${LOCAL_OPTS[@]}"; status=$?
        fi
        # Another run (a manual ci_local.sh) held the lock: try again next time.
        [[ $status == 3 ]] && { log "ci_local.sh is busy; $ref waits"; continue; }
        [[ $status == 0 ]] && result=PASS || result=FAIL
        echo "$key" >"$DIR/state/$name"
        echo "$(date '+%F %T') $result $ref ${key:0:7} main ${main:0:7} $logs" >>"$DIR/history.log"
        log "$result $ref (logs: $logs)"
        notify "$result: $ref"
    done
    # Old runs go; history.log stays.
    ls -1d "$DIR"/runs/*/ 2>/dev/null | head -n -"$KEEP_RUNS" | xargs -r rm -rf
}

what="main"; [[ $BRANCHES == 1 ]] && what+=" and branches '$GLOB'"
log "watching origin ($what), every ${INTERVAL}s; history: $DIR/history.log"
while true; do
    check
    [[ $ONCE == 1 ]] && break
    sleep "$INTERVAL"
done
