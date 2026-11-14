# 801-ASR-0014 — The stage's conditions as checks that run on every round

**Type:** assessment (tests) · **Owner:** `ASR` · **Priority:** P1 · **Status:** open (2026-11-14) · **Blocked by:** the parts of [801](801-the-bench-stays-manageable.md) each case checks · **Main task:** [801](801-the-bench-stays-manageable.md) · **Constitution:** MC-12.1, MC-12.2

## Problem

What broke on the bench on 2026-11-13 and 14 had no check in the gate. Each case the stage names needs one, so that a later change cannot bring it back unseen.

## Plan

**A QEMU suite, `manageable`,** with the Effector server's fixture and a slot image. Each case ends with the agent connected, or rolled back to the slot that had it with its S2 report:
1. a release whose network never comes up (a stand-in: its `netstack` refused): rolled back, and the report names the step;
2. the RTC set 3 h ahead before boot: commands are taken, and the clock is right after the network;
3. a platform table the release cannot read (a stand-in table in QEMU's ACPI): the radio sends as before;
4. remembered networks kept in an older format: still there after the trial, whether it is confirmed or rolled back;
5. the previous round's release updated to this one through the server (S4, S7).

## Acceptance criteria

- The suite is a CI group in the gate, on x86 and aarch64.
- Each case fails when its part is taken out.

## Related

[801](801-the-bench-stays-manageable.md).
