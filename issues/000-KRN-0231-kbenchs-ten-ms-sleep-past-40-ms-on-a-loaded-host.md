# 000-KRN-0231 — `kbench`'s 10 ms sleep past 40 ms on a loaded host

**Type:** tests (`kbench_check`) and the kernel's sleep · **Owner:** `KRN` · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** — · **Roadmap:** track A · **Constitution:** MC-12.1, MC-12.2

## Problem

`kbench_check` (`tests/qemu_smoke.py`, the `bench` suite in "x86: keys, shell, tools") asks that `sleep 10 ms`'s median be at most 40 ms. On 2026-11-10 it failed four times:

| when | tree | machine | `sleep 10 ms` median / longest | `sleep 1 ms` median |
|---|---|---|---|---|
| round 112's gate | candidate a9132d66 | i9 | 47.7 / 67.7 ms | 1.85 ms |
| round 112, alone | candidate a9132d66 | i9 | 47.7 ms | — |
| by hand | `main` 06f3a8b0 | i9 | 47.0 / 66.2 ms | 1.92 ms |
| round 114's gate | candidate 60ba280e | acc-2 | over 40 ms (the line was cut) | — |

- **The same groups passed elsewhere:** on acc-1 (477 s, round 112's candidate), on acc-2 when round 114's gate ran it again alone (519 s), and on i9 in rounds 102 to 110.
- **So a 10 ms sleep takes 40 to 70 ms in QEMU on some hosts at some times,** while a 1 ms sleep stays near 2 ms.

**Not known:** whether the time is the host's (QEMU's timer under TCG on a busy or coarse host) or the kernel's (how a sleep longer than a tick is woken). This is not a root cause (MC-12.2).

## Plan

1. **The kernel's side.** Read how a sleep of more than one tick is armed and woken on x86 (the 1 ms tick, 000-KRN-0083). Then time it in QEMU on an idle host and beside a loaded one: the requested sleep, the tick count, the wake.
2. **Fix what is the kernel's.** If a sleep of n ticks waits for later ticks than the n-th, fix it. If the host's timer is what lags, the check keeps its bound and the gate keeps such a host out, saying so in `LANDING/`, as round 113 did with i9.
3. **The bound is not widened** to make a run pass (MC-12.5).

## Acceptance criteria

- The cause is named, with the measurements that show it.
- "x86: keys, shell, tools" passes ten times in a row on acc-2 beside a loaded lane.

## Related

000-KRN-0065 (a sleep never shorter than asked), 000-KRN-0083 (the 1 ms tick on x86), round 112's and round 114's gate logs, `HUMAN/2026-11-10T1426Z-KRN-i9s-timer-…`.
