# 000-NET-0056 — The TLS lending check lists the grant while its program runs

**Type:** test · **Owner:** kernel session (network track) · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Roadmap:** the landing gate · **Constitution:** MC-12.2

## Problem

**What failed.** Round 74's gate on x32-128 (candidate 7fd66f5d, 2026-11-04) failed in `x86: network, TLS and Effector`. The TLS suite's `tls_lending_check` read `netgrants` and got `NO NETWORK GRANTS`.

**Why.** The check ran `netgrants` after it had killed `tlsclock`. `netpolicy` drops the grant of a process that ended at its next sweep (`expire`). So the check passed only when the sweep came after `netgrants`'s answer.

**The candidate is not the cause.** None of its changes touch grants or their sweep. The race is in the test since the check came in.

## Plan

1. **`lent()`** takes a check to run while the program is still alive: after `stat caps`, before the kill.
2. **The grant check** runs there and names the program: `tlsclock RULES=`, where it was any `GRANT `.

## Acceptance criteria

The TLS suite passes with the check before the kill, on x86 in QEMU, locally and in a gate.

## Related

000-APP-0115 and 351-APP-0083, where the check came from; round 74's gate log.
