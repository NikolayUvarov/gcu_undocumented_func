#!/usr/bin/env bash
# Copies this repository's main and tags to the showcase repository, mind-core-os (000-KRN-0088). Work stays here; the
# showcase is only pushed to, and only when main or the tags changed. .github/workflows/mirror.yml runs it once a day
# with --every-hours 48, so the showcase changes at most every 48 hours (the maintainer, 2026-10-11); anyone with
# write access to the showcase may run it by hand. Never forced: a commit made in the showcase stops the copy (the
# push is refused and this script fails) instead of being overwritten.
#
#   scripts/mirror_showcase.sh [--every-hours N] [SHOWCASE_URL]
#
# --every-hours N: copy only if the showcase's last push (GitHub's pushed_at) is at least N hours old.
# SHOWCASE_URL: default https://github.com/NikolayUvarov/mind-core-os.git (the workflow passes its SSH address).
# MIRROR_SOURCE: the remote to copy from (default origin). MIRROR_REPO: the clone to work in (default this one).
# MIRROR_LAST_PUSH: the showcase's last push (ISO 8601) instead of asking GitHub, for tests.
set -euo pipefail
hours=0
if [[ ${1:-} == --every-hours ]]; then hours=$2; shift 2; fi
REPO=${MIRROR_REPO:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}
SOURCE=${MIRROR_SOURCE:-origin}
TARGET=${1:-https://github.com/NikolayUvarov/mind-core-os.git}
git=(git -C "$REPO")

# main into a ref of its own, so the clone's branches are left as they are.
"${git[@]}" fetch -q "$SOURCE" +refs/heads/main:refs/mirror/main --tags
main=$("${git[@]}" rev-parse refs/mirror/main)
shown=$("${git[@]}" ls-remote "$TARGET" refs/heads/main | cut -f1)
new_tags=$(comm -23 <("${git[@]}" tag | sort) <("${git[@]}" ls-remote --tags --refs "$TARGET" | sed 's#.*refs/tags/##' | sort))
if [[ $main == "$shown" && -z $new_tags ]]; then
    echo "mirror: nothing changed since the last copy (main at ${main:0:7})"
    exit 0
fi
if ((hours > 0)); then
    last=${MIRROR_LAST_PUSH:-}
    if [[ -z $last ]]; then
        repo=$(sed -E 's#^(https://([^@/]*@)?github\.com/|git@github\.com:)##; s#\.git$##' <<<"$TARGET")
        auth=(); [[ -n ${GITHUB_TOKEN:-} ]] && auth=(-H "Authorization: Bearer $GITHUB_TOKEN")
        last=$(curl -fsS "${auth[@]}" "https://api.github.com/repos/$repo" | python3 -c 'import json, sys; print(json.load(sys.stdin)["pushed_at"])')
    fi
    age=$((($(date -u +%s) - $(date -u -d "$last" +%s)) / 3600))
    if ((age < hours)); then
        echo "mirror: changes wait: the last copy is ${age} h old, less than ${hours} h"
        exit 0
    fi
fi
"${git[@]}" push -q "$TARGET" refs/mirror/main:refs/heads/main
"${git[@]}" push -q "$TARGET" --tags
echo "mirror: $(sed -E 's#//[^@/]*@#//#' <<<"$TARGET") has main at ${main:0:7} and $("${git[@]}" tag | wc -l) tags"  # no credentials in the log
