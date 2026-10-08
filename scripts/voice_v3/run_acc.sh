#!/bin/bash
# run_acc.sh <work> <jobs> <model:lang>...: accuracy runs, <jobs> at a time, logs in <work>/logs.
W=$1; J=$2; shift 2; E=$(dirname "$0")/evaluate.py; mkdir -p "$W/logs"
printf '%s\n' "$@" | xargs -P "$J" -I{} bash -c 'm=${1%%:*}; l=${1##*:}; ${PYTHON:-python3} -I "$2" "$3" $m $l > "$3/logs/$m.$l.log" 2>&1 || echo "FAIL $1"' _ {} "$E" "$W"
