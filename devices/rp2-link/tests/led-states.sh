#!/usr/bin/env bash
# Shows each of the multi-device's LED states in turn on one board, through its control port (800-LNK-0005): the
# boot indicator's lines as MIND Core would send them, with a pause to look at each colour.
# Usage: devices/rp2-link/tests/led-states.sh <control port> [seconds per state, default 3]
#   the control port: /dev/serial/by-id/usb-MIND_Core_MIND_Link_<serial>-if04 (Linux, WSL with usbipd)
# Needs mindlink (tools/mindlink): $MINDLINK, else the release build beside this tree, else mindlink on $PATH.
set -euo pipefail
PORT="${1:?usage: $0 <control port> [seconds]}"
STEP="${2:-3}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
ML="${MINDLINK:-$ROOT/tools/mindlink/target/release/mindlink}"
[ -x "$ML" ] || ML=mindlink

# A state's lines, then a beat each second while it is looked at (3 s without one shows as the system stopping).
BEAT=0
show() {
    echo "$1"; shift
    for line in "$@"; do "$ML" "$PORT" board "$line" >/dev/null; done
    for _ in $(seq "$STEP"); do sleep 1; BEAT=$((BEAT + 1000)); "$ML" "$PORT" board "@BEAT $BEAT" >/dev/null; done
}

"$ML" "$PORT" board hello
echo "Before any indicator line the LED shows the link: dim cyan with no other board, dim green with one."
sleep "$STEP"
show "yellow, dim: KERNEL" "@STATE KERNEL"
for n in 5 15 25 30; do show "yellow, brighter: SERVICES $n/30" "@STATE SERVICES $n/30"; done
show "green: READY, HEALTH OK" "@STATE READY" "@HEALTH OK"
show "orange: HEALTH WARN" "@HEALTH WARN a test"
show "red: HEALTH FAIL" "@HEALTH FAIL a test"
"$ML" "$PORT" board "@HEALTH OK" >/dev/null
echo "magenta blinking: the beats stop (it comes after 3 s)"
sleep $((STEP + 3))
echo "white flashes: REBOOT"; "$ML" "$PORT" board "@STATE REBOOT" >/dev/null; sleep "$STEP"
echo "white flashes: OFF"; "$ML" "$PORT" board "@STATE OFF" >/dev/null; sleep "$STEP"
echo "Done. Unplug and plug the board back to clear the indicator's state."
