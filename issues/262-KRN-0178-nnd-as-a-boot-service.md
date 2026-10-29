# 262-KRN-0178 — `nnd` as a boot service: its grants, its memory line and workers, and the shell's badged client

**Type:** kernel (ABI: a boot image and a slot), `init`, the shell's launch · **Owner:** `KRN` · **Priority:** P3 · **Status:** open (waits for the design to be agreed) · **Blocked by:** the agreement on [docs/nn/runtime.md](../docs/nn/runtime.md) (262-APP-0112, NRL's answers) · **Main task:** [262](262-model-runtime-on-the-machines-full-hardware.md) · **Constitution:** MC-3.1, MC-3.2, MC-8.1, MC-12.4

**Asked by:** the tools track (`APP`) in `issues/requests-KRN.md`, 2026-10-28, "The model service `nnd` (for 262-APP-0112)".

## Problem

- **Who it serves.** `nnd` serves models to programs and to the shell (262's stage 2).
- **What it lacks.** It needs the following, and none exists yet:
  - a place among the boot services;
  - read-only scopes of the models;
  - a memory line of its own with large pages;
  - the right to start workers;
  - a client identity per program.

## Plan

1. **A boot image `nnd`** after `vfs_server` and `logd` (an ABI step with its own version).
2. **`init`'s grants, nothing else:**
   - a read-only scope of `models:` and one of `data/models/`;
   - `logd`;
   - its own endpoint.

   No network, no lifecycle client, no device.
3. **Its memory and workers** (the answer to the design's question 3):
   - **The line.** `init` reads the same `data/memory.conf` the loader reads for programs (262-KRN-0160, 0163). A line `nnd <MiB>` gives the boot service its quota and the right to workers (`SPAWN_WORKERS`) at each start and restart.
   - **The default.** Without a line, `nnd` gets the default quota and no workers.
   - **The limits.** The same ceiling as for programs applies, and large pages come from the quota (262-KRN-0161).
4. **The shell's client of `nnd`:**
   - a slot of its own, held with `CAP_BADGE`;
   - for a program that asks with a new `REQUEST_NN` bit, the shell lends a copy with a badge minted for that start alone (000-KRN-0073);
   - the badge is the client's identity in `nnd`.
5. **A badge is reused only after its retirement is complete** (NRL's point, 2026-10-29; proposed to APP, whose shell and `nnd` carry most of it):
   - **The problem.** "Not while the program runs" is not enough. A copy the program passed on, a child it started, or `nnd`'s own sessions, jobs and lent memory for that badge can outlive the program, and a reused badge would make a new client their owner.
   - **What the kernel already gives.** A copy of a capability is a child of its source, and `CAP_REVOKE` removes every descendant from every task (000-KRN-0104). An endpoint's senders are served in order (000-KRN-0010).
   - **The shell (APP):**
     - mints the badged client per start and **keeps the minted capability** (today it drops a minted copy once the loader holds it);
     - when the program ends, `CAP_REVOKE` on it removes every copy anyone holds; then it drops it;
     - then it sends `nnd` a retirement of that badge on its own unbadged client, which only the shell holds; that message comes after anything the badge's holders sent;
     - only once `nnd` has answered does the badge go back among the free ones, and the lowest free badge is taken first, so that a test can see a badge reused;
     - when the badges run out, a program asking for `nnd` is refused, not given a badge still owed.
   - **`nnd` (APP):** takes a retirement only from badge 0 (the shell's unbadged client). It drops that badge's sessions, jobs and lent memory before it answers; a retirement of every badge comes at the shell's start.
   - **`init` (this task):**
     - gives the shell a copy of a capability it keeps for that purpose;
     - when the shell is restarted, revokes that capability's descendants first, so that no copy the old shell minted survives;
     - the new shell's retirement of every badge then clears `nnd`.
6. **A wait and wake on a shared word** (the design's question 4) is not planned in this task. It is its own kernel task, [262-KRN-0179](262-KRN-0179-a-wait-on-a-shared-word.md). Until it lands, `nnd` starts a worker per job, as the design says.

## Acceptance criteria

- **Host tests:**
  - `tests/boot_abi_host.rs`: the image and its order;
  - `tests/runtime.rs`: the slot and `REQUEST_NN`;
  - `init`'s reading of `data/memory.conf` for a service.
- **QEMU, x86 and aarch64:**
  - `nnd` starts with the scopes and nothing else (`stat caps`);
  - its memory line is applied, and a missing line gives the default;
  - a program with `REQUEST_NN` gets a badged client and one without gets none;
  - a restart of `nnd` keeps the line;
  - **a client's exit and its badge's reuse:**
    - a program with `REQUEST_NN` opens a session and passes a copy of its client to a child, then ends;
    - the child's copy is gone (its call has no capability);
    - the next program gets the same badge and is refused the old session (`not-found`, not `owner`);
    - a restart of the shell leaves no client of the old one's.
- **The native run** that closes NRL's T11 (900-NRL-0017) is APP's and NRL's.

## Related

[262-APP-0112](262-APP-0112-nnd-and-nn-wit-stage-0.md), [262-KRN-0179](262-KRN-0179-a-wait-on-a-shared-word.md), 262-KRN-0160, 262-KRN-0161, 262-KRN-0163, 000-KRN-0073, 900-NRL-0017.
