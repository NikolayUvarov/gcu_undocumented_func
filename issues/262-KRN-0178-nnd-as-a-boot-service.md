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
5. **A wait and wake on a shared word** (the design's question 4) is not planned in this task. It is its own kernel task, [262-KRN-0179](262-KRN-0179-a-wait-on-a-shared-word.md). Until it lands, `nnd` starts a worker per job, as the design says.

## Acceptance criteria

- **Host tests:**
  - `tests/boot_abi_host.rs`: the image and its order;
  - `tests/runtime.rs`: the slot and `REQUEST_NN`;
  - `init`'s reading of `data/memory.conf` for a service.
- **QEMU, x86 and aarch64:**
  - `nnd` starts with the scopes and nothing else (`stat caps`);
  - its memory line is applied, and a missing line gives the default;
  - a program with `REQUEST_NN` gets a badged client and one without gets none;
  - a restart of `nnd` keeps the line.
- **The native run** that closes NRL's T11 (900-NRL-0017) is APP's and NRL's.

## Related

[262-APP-0112](262-APP-0112-nnd-and-nn-wit-stage-0.md), [262-KRN-0179](262-KRN-0179-a-wait-on-a-shared-word.md), 262-KRN-0160, 262-KRN-0161, 262-KRN-0163, 000-KRN-0073, 900-NRL-0017.
