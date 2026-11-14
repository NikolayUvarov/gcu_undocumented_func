# 801-LNK-0045 — A rollout one machine at a time, and the RS-485 line on all three

**Type:** tools (`scripts/`, `effector485`) · **Owner:** `LNK` · **Priority:** P1 · **Status:** open (2026-11-14) · **Blocked by:** S1 of [801](801-the-bench-stays-manageable.md) for the gate on the agent's return · **Main task:** [801](801-the-bench-stays-manageable.md) · **Constitution:** MC-9.3, MC-12.1

## Problem

On 2026-11-14 a release went to three machines one after another, and the third lost its network. Nothing stopped the rollout before it.

The RS-485 cable was moved by hand between the machines.

## Plan

1. **`scripts/rollout.py`:**
   - takes one release and a list of machines;
   - sends it to the first through the Effector server, or over the cable when the server cannot reach it;
   - waits for that machine's agent to come back on the new release;
   - only then goes on to the next.

   A machine that falls back, or whose agent does not come back within the bound, stops the rollout and shows its report (S2).
2. **Several machines on one line** ([800-LNK-0026](800-LNK-0026-several-machines-on-one-line.md)) on the bench:
   - an address per machine in `link.conf`;
   - `effector485 --addresses`;
   - each machine named by its `agent`.

   The cable then stays where it is.

## Acceptance criteria

- The rollout stops at a machine whose release is rolled back, and names the reason.
- On the bench, the three machines are reached on one line by their addresses.

## Related

[801](801-the-bench-stays-manageable.md), [800-LNK-0026](800-LNK-0026-several-machines-on-one-line.md).
