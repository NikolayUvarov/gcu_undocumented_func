# 262-KRN-0178 — `nnd` as a boot service: its grants, its memory line and workers, and the shell's badged client

**Type:** kernel (ABI: a boot image and a slot), `init`, the shell's launch · **Owner:** `KRN` · **Priority:** P3 · **Status:** in progress (2026-10-30: `init`'s part made; APP's shell part left) · **Blocked by:** — (the design, [docs/nn/runtime.md](../docs/nn/runtime.md), is at 1.0, agreed 2026-10-29) · **Main task:** [262](262-model-runtime-on-the-machines-full-hardware.md) · **Constitution:** MC-3.1, MC-3.2, MC-8.1, MC-12.4

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

## Progress (2026-10-30)

- **The ABI step, ABI 25 and handoff 25** (`common/abi.rs`):
  - the boot image `nnd` after `sysmon`, the 45th;
  - `SLOT_NN` 41, in `LAUNCH_SLOTS`, moves `SLOT_DYNAMIC` to 42, and `CAP_SLOTS` grows to 106;
  - `REQUEST_NN` (`1 << 27`) in `mind::process`.
- **`init`** (`init/src/main.rs`):
  - `nnd` holds its endpoint, the log and two read-only scopes, made at each start where each exists:
    - `data/models/` in `SLOT_VFS`;
    - the model disk's root, `models:`, in slot 2.

    No network, device or lifecycle client.
  - **`init` makes neither directory.** At first it made `data/models/` at each boot, and the first run on the machines failed twice:
    - a boot test found the FAT's dirty bit set on its volume, x86 and aarch64;
    - `fm`'s listing of `/` began with the new `data`.

    So a boot writes nothing to the boot volume for `nnd`, and its `SLOT_VFS` scope comes once the owner makes `data/models/`.
  - At each start and restart `init` reads `nnd`'s line of `data/memory.conf` (`mind::memconf`, which the loader now reads through too). With a line, `nnd` gets:
    - its MiB as its quota;
    - the argument `--memory <MiB>`;
    - `SPAWN_WORKERS`;
    - a task quota of 32 (`NND_TASKS`), a ceiling: the kernel bounds its live workers by the online CPUs too, and `init`, which starts before the other CPUs, cannot count them.

    Without a line, the default 16 MiB, no argument and no workers; one endpoint either way, for the probe's notices.
  - The shell holds a copy of a client of `nnd` with the right to badge, which `init` keeps, in `SLOT_NN`. Before each new shell, `init` revokes every copy of it: the old shell's, what it minted and lent. Each revocation is logged.
  - `nnd` may be turned off in `data/services.txt`, and the shell starts after it.
- **The kernel gives a task the online CPUs' count** in `BootInfo.cpu_count`, as many as were online at its start. It gave 0 before, and nnd, which sizes its workers by it, saw one CPU (the first run of `nnd_check`: 1 worker on 4 CPUs). No CPU's address is given.
- **Answers to APP** (its message of 2026-10-29):
  - `--memory <MiB>` suits: `init` passes it with the line, and nothing without one.
  - **The files are two scopes, not one client.** A `vfs_server` scope is one directory, so:
    - `SLOT_VFS` holds `data/models/`, whose root stands for that directory;
    - slot 2 holds `models:`.

    `nnd` opens `data/models/…` through the first and `models:…` through the second (`mind::fs::use_scope` with each base).
- **Tests:**
  - **Host:** `tests/boot_abi_host.rs` (the image's place and the handoff); `tests/runtime.rs` (the slot and the launch slots); `tests/services_host.rs` (the memory line).
  - **QEMU:** the 4-CPU suite's `nnd_check` (`tests/qemu_smoke.py`, also `--suites nnd`) checks:
    - the grants (`stat caps`);
    - the quota and workers with and without the line, read again at a restart and after nnd's end;
    - the shell's `SLOT_NN`.
- **Left:**
  - **APP's shell part:** the badge per start, kept and revoked at the program's end, `retire`, and `retire-all` at its start. With it come the criteria about `REQUEST_NN`, a badge's reuse and a restart of the shell.
  - **`nnd`'s two scopes,** for loading models.

## Landed (2026-10-30)

In `main` since landing round 50 (8e2b16e0, 2026-10-30), with ABI 25 and handoff 25. Left as above: APP's shell part (the badge per start and its revocation) with the criteria about `REQUEST_NN`, a badge's reuse and the shell's restart; `nnd`'s two scopes; NRL's and APP's native run.

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
