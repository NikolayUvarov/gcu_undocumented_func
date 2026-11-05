# 000-KRN-0208 — A `run` right after forty `kill`s waited past 8 s for the shell (seen once)

**Type:** shell, `init` or the kernel (to find) · **Owner:** kernel session · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Roadmap:** the landing gate · **Constitution:** MC-12.2

## Problem

**Seen once.** Round 79's gate (candidate a54a79c9, `main` with DRV's `regdb`), `x86: RAM above 4 GiB` on x32-128, the `normal` suite, `-m 6G`.
- The suite starts 40 `memtest hold 0` programs, reads `stat tasks`, and kills the 40.
- At once it runs `run memtest endpoints &`. The shell printed nothing for 8 s after the echo, so the check timed out.
- The group passed when the gate ran it again alone, on the same lane (1072 s). DRV's change could not touch it: `regdb` is a crate no image program uses.

**What is not known:** what the shell waited for. Readings that fit:
- the 40 exits' notices and the revocation of each start's minted clients (262-KRN-0178);
- the loader;
- the kernel retiring 40 tasks.

This is not a root cause (MC-12.2).

## Plan

1. **Reproduce:** the check alone, 20 times, on x32-128, with the shell's and `init`'s logs kept. Then do the same with the kills followed by the run at once and the run delayed.
2. **Find what the shell waits for,** from its log and `stat`. Fix the wait if it is one, or make the check wait for the exits it caused, if the wait is right.

## Acceptance criteria

- The cause is named, with the log that shows it.
- The check passes 20 times in a row on x32-128.

## Progress

2026-11-05, plan step 1 in part, on `main` f81cd4d9 (round 84):
- **The run:** `tests/qemu_smoke.py --memory 6G --suites normal`, 20 times in a row, on x32-128 in lane `KRN-2`.
- **Beside it:** lane `KRN` repeated the x86 updater group for 000-KRN-0210, as the gate's other lane did in round 79.
- **All 20 passed,** in 170 to 173 s each. So the forty kills and the `run` at once never waited 8 s. The second criterion is met, on that machine and that load (MC-12.1).
- **Not seen, so not named:** the cause of round 79's wait (MC-12.2).
- **What differed from round 79:** its group ran `normal` first, then `display`, `net`, `vfs` and `fill`, and its neighbour was another gate group, not the updater's.

**Left:**
- the cause, if the wait comes back. The gate keeps the logs of a group that fails (000-KRN-0210's `bounded`);
- the run delayed after the kills, from plan step 1, only if it does.

## Related

262-KRN-0178 (a badge reused only after its retirement), round 79's gate log.
