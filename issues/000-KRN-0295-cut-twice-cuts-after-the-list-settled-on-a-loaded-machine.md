# 000-KRN-0295 — `cut_twice` can cut after the safe start's list settled, on a loaded machine

**Type:** test (`tests/qemu_smoke.py`) · **Owner:** `KRN` (the kernel session) · **Priority:** P2 · **Status:** open (2026-11-28) · **Blocked by:** — · **Main task:** — (for [000-KRN-0217](000-KRN-0217-a-safe-start-that-leaves-out-only-what-it-suspects.md)'s check) · **Constitution:** MC-12.2

## Problem

DRV's report `KRN/2026-11-28T1050Z-DRV-re-213-krn-0290-the-maintainers-look-every-letter-whole-the-pointer-smooth.md` on `internal-dev-messages`.

On LEG9, at bench-53's commit, "x86: boot, display, shell, memory, clock" failed once, in the power suite's `cut_twice`:
- the line `[INIT] THE LAST RUN DID NOT END AS ASKED, NO SIGN OF A FREEZE: …` was not in `dmesg -s init` after the first cut;
- LEG9 was building another tree at the time;
- the power suite then passed alone.

No such failure has been seen in this session's gates before.

**The likely cause (inferred from the code, not shown).**
- `cut_twice` resets the machine when it sees the shell's prompt. It counts on that being within `safe::SETTLE_MS` (30 s) past READY, before the list's starts settle.
- On a loaded machine the prompt can come later than that. Then init has already said `EVERY START OF THE SAFE START'S LIST SETTLED: A RUN CUT NOW LEAVES NOTHING OUT`.
- So the run was not cut while it started anything, and the next boot rightly suspects nothing. The check's timing failed, not init.

A failing test is not called a flake: the check must say which of the two it saw.

## Plan

1. **Before each cut, `cut_twice` reads the console it has seen since `INIT STARTED`:**
   - if init said its list settled, the cut would be late. The check says so and starts that boot again, at most three times, before it fails with that reason;
   - otherwise it cuts as now.
2. **The check's failure message** carries the console's lines from READY to the cut, so that a late cut and a lost line can be told apart.
3. **The second cut** takes the same guard.

## Acceptance criteria

- [ ] Shown by a run made slow on purpose (for example `-icount` or a CPU limit on the QEMU process): the check finds the late cut and boots again, where today it fails.
- [ ] The power suite passes on both architectures, alone and beside a loaded lane (10 runs each), and in a gate.

## Related

- [000-KRN-0217](000-KRN-0217-a-safe-start-that-leaves-out-only-what-it-suspects.md) (the safe start and `cut_twice`).
