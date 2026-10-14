# 000-KRN-0104 — `CAP_REVOKE` bounded as MC-5.4 asks, and the profile says how

**Type:** kernel · **Owner:** kernel session · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** — · **Constitution:** MC-3.5, MC-3.6, MC-5.4, MC-12.3

## Problem

From the critique of 2026-10-14 ([issues-critica-done/01](../issues-critica-done/01_constitution_and_docs_analysis.md), §1.1), checked against the code.

- **What the Constitution asks.**
  - MC-3.6: a revocation has a declared scope and an observable completion; after it, no new operation goes through what it revoked.
  - MC-5.4: long revocation, cleanup and scanning are bounded, splittable or preemptible, as the profile says.
- **What the kernel does.** `Scheduler::revoke` (`kernel/src/scheduler.rs`):
  - repeats passes over every slot of every task until a pass removes nothing;
  - checks each slot's parent against the nodes found so far by a linear search;
  - allocates a list of every slot in the system from the kernel heap;
  - does all of it under the scheduler lock, with interrupts off on that CPU.

  Its cost grows faster than the number of slots in the system, and only the 4096 slots of a task and the memory quotas bound it. Nothing splits the walk or lets it be preempted. The caller waits (`BlockedFlush`) only for other CPUs to leave the affected address spaces.
- **What the profile says.** The MC-3.4–3.6 and MC-5.1–5.5 rows of [docs/profile](../docs/profile/README.md) are silent on revocation's cost. [clocks.md](../docs/profile/clocks.md) says no deadline or jitter bound is claimed for MC-5.3 and MC-5.4.
- **What the critique got wrong.** The Constitution does not contradict itself. MC-3.6 does not ask for a revocation in one step, and MC-5.4 leaves the bound to the profile. What is missing is the profile's statement and a walk that keeps to it.

## Plan

1. **Measure.** A `kbench` row: `CAP_REVOKE` of a subtree of 10, 100, 1000 and 10 000 nodes spread over several tasks, with the time the scheduler lock is held.
2. **State it now.** The MC-3.4–3.6 row says how revocation's cost grows and that MC-5.4 is not met for it (000-KRN-0105 writes the line; this task updates it once the walk is bounded).
3. **Walk only the subtree.** Each node keeps links to its children (or a parent-indexed table), so a revocation visits the nodes it removes and no others.
4. **Split the walk.** A bounded number of nodes per step, the lock released between steps:
   - the root is marked as being revoked first;
   - while it is, a copy, mint or transfer of any node under it is refused (an error the ABI names; a new error code is an ABI change in this track);
   - the call returns when the last step is done. That is the observable completion of MC-3.6, and MC-3.5's "fate of operations in progress" is the refusal.
5. **The model.** [CapRevokeMove.tla](../docs/assurance/CapRevokeMove.tla) gets the state "being revoked" and the refusal, and the model check passes.
6. **The profile.** The MC-3.4–3.6 and MC-5.1–5.5 rows state the bound per step and the configuration it was measured on.

## Acceptance criteria

- [ ] The `kbench` row shows a revocation's time growing with the nodes it removes, not with the slots in the system (x86 and aarch64, QEMU).
- [ ] No step holds the scheduler lock for more than the stated number of nodes. A test revokes a large subtree on one CPU while another CPU's clock keeps its tick (the `smp` suite).
- [ ] A copy, mint or transfer from a subtree being revoked is refused, in the model and in the `isolation` suite.
- [ ] docs/profile states the bound, with its evidence and configuration (MC-12.1).

## Related

000-KRN-0105 (the profile's interim line), [CapRevokeMove.tla](../docs/assurance/CapRevokeMove.tla), issue 171 (capability tables from memory).
