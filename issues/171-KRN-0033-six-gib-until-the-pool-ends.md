# 171-KRN-0033 — Spawning until the frame pool ends on a 6 GiB machine

**Type:** kernel (test) · **Owner:** `KRN` · **Priority:** P3 · **Status:** open · **Blocked by:** — · **Main task:** [171](../issues-done/171-limits-from-the-hardware.done) · **Constitution:** MC-1.7, MC-12.1

Split from [171-KRN-0032](../issues-done/171-KRN-0032-kernel-structures-in-the-frame-pool.done), whose second machine size this is.

## Problem

171-KRN-0032 moved each task's kernel structures to the frame pool. On 512 MiB machines (x86 and aarch64) the `normal` suite shows that clocks run until the frame pool, not the arena, runs out, at 16–17 bytes of arena a clock.

On a 6 GiB machine that takes about 1400 clocks with 4 MiB screens. Under TCG a machine drawing that many clocks hardly runs, so the suite skips it above 1 GiB. That the arena no longer bounds a large machine is therefore arithmetic from the 512 MiB numbers, not a test.

## Plan

- A cheaper program for the count: a console program that starts, takes a little heap and waits without waking (no screen, no timer). Started in the background until the loader refuses.
- Check, at 6 GiB on x86 and aarch64, that the refusal comes from the frame pool (under the reserve and one program left) while the arena has grown by less than 4 KiB a task.
- Run it in a CI group of its own if it fits the time, or in the local gate only.

## Acceptance criteria

On QEMU with 6 GiB, x86 and aarch64, programs start until the frame pool runs out, and the arena stays below its limit with room to spare.

## Related

[171-KRN-0032](../issues-done/171-KRN-0032-kernel-structures-in-the-frame-pool.done).
