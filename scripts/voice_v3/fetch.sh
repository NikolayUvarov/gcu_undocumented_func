#!/bin/bash
# fetch.sh <dir> <url>...: each download into its own directory, retried; an existing file is kept.
D=$1; shift; mkdir -p "$D"
for u in "$@"; do
  f="$D/$(basename "${u%%\?*}")"
  [ -s "$f" ] && continue
  for i in 1 2 3 4; do curl -sSL --fail --retry 3 -o "$f.part" "$u" && mv "$f.part" "$f" && break; sleep $((i*2)); done
  [ -s "$f" ] || echo "FAILED $u"
done
