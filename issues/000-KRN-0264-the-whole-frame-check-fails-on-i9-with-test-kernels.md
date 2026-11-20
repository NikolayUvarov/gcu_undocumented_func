# 000-KRN-0264 — The whole-frame check fails on i9 with the test kernels: the first screen's top 4–5 frames behind its bottom

**Type:** the `isolation` suite's frame check (000-KRN-0086), the compositor · **Owner:** `KRN` · **Priority:** P2 · **Status:** open (2026-11-20) · **Blocked by:** — · **Roadmap:** track A · **Constitution:** MC-12.2

## Problem

The `isolation` suite checks that the compositor copies whole frames. `app2` numbers each frame in its colour and draws the bottom half 3 ms before the top, and the check reads screens with the guest paused. A bottom half newer than the top means a frame copied while it was drawn.

**The failures.** The check failed twice in two rounds, on i9 alone, each time in a group with a test kernel. Both times the failure came on the first screen, with the bottom at 6. Each group passed when it ran again alone on x32.

| Round | Group | Kernel | i9 | The rerun on x32 |
|---|---|---|---|---|
| 154 | x86: padded vector area | the padded vector area's | `top 2, bottom 6; [(2, 6)]` | PASS, 128 s |
| 155 | x86: x2APIC | `mind-x2apic-target` (`-cpu max`) | `top 1, bottom 6; [(1, 6)]` | PASS, 224 s |

**It did not happen before these rounds:**
- in rounds 151 and 152, x2APIC passed on i9 and acc-1;
- in round 152, padded vector area passed on i9;
- `x86: scheduling, isolation, heap`, with the normal kernel, passed in all four rounds.

**What changed between.** Round 154 landed 801-KRN-0261: the kernel's stops and a firmware variable read at boot. Nothing in it runs while frames are copied, unless a fault comes. Whether it, the load on i9, or QEMU's screen read is the cause is not known.

## Plan

1. Repeat the two groups ten times each on i9, alone and beside a loaded lane. Do the same on `main` before round 154 (`9bff1e81`) and after it, so that the change and the machine are told apart.
2. On the first failing screen, keep the whole picture, not only the two rows, to see whether the top is a stale frame or a torn one.
3. Fix whatever the runs name: the compositor, the check's first screen, or the harness's paused read.

## Acceptance criteria

- [ ] The cause named from runs, or the check made to tell a stale screen from a torn frame, with the reason.
- [ ] Ten runs of each group on i9 pass after the fix.

## Progress (2026-11-20)

**Alone on i9, after round 154** (`main` `20a8df9e6c`). Each group ran five times, alternating, with no other KRN operation on the machine. All ten passed:
- `x86: x2APIC`: 292 to 295 s;
- `x86: padded vector area`: 151 to 153 s.

**So the failure needs company.** In the two gates, i9 ran one KRN lane beside other tracks' operations on the same machine. Without them it did not come back.

**Left:**
- runs beside a loaded i9, or with QEMU's CPU time cut, to bring it back;
- the whole picture of a failing first screen.

## Related

000-KRN-0086 (the whole-frame copy), [801-KRN-0261](../issues-done/801-KRN-0261-every-stop-of-a-trial-falls-back-and-is-recorded.done) (round 154), [000-KRN-0233](000-KRN-0233-the-compositor-compares-every-frame-of-an-idle-shell.md).
