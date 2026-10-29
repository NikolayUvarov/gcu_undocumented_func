# 262-APP-0112 — The model service `nnd` and `idl/nn.wit` 1.0: stage 0 of 262, for NRL's native integration

**Type:** tools (a service, IDL, the shell) · **Owner:** tools track (`APP`) · **Priority:** P1 · **Status:** open (the design, [docs/nn/runtime.md](../docs/nn/runtime.md), is at 1.0, agreed on 2026-10-29 with `NRL` and `KRN`; the code follows) · **Blocked by:** for the native part, `nnd` as a boot service with its grants and the shell's badged client ([262-KRN-0178](262-KRN-0178-nnd-as-a-boot-service.md), which the agreed design unblocks); a kept worker, a wait on a shared word ([262-KRN-0179](262-KRN-0179-a-wait-on-a-shared-word.md), planned; until then a worker per job). `nrl-runtime` is in `main` since 2026-10-28; `NRL`'s pinned test model and references (900-NRL-0025) are in its landing request · **Main task:** [262](262-model-runtime-on-the-machines-full-hardware.md) · **Roadmap:** track G · **Constitution:** MC-3.1, MC-3.13, MC-5.1, MC-5.2, MC-5.5, MC-8.1, MC-11.5, MC-11.11, MC-12.1, MC-12.2, MC-12.3, MC-12.4

**Recorded by:** the tools track, 2026-10-28, from NRL's request `issues/requests-APP.md#nrl-native-nnd-integration-for-900-t11` on `codex/NRL-lab` (41633c21) and its messages `APP/2026-10-26T1740Z-NRL-native-nnd-interface-and-smoke-harness.md` and `APP/2026-10-26T1828Z-NRL-runtime-v0-ready-native-integration-pending.md` on `internal-dev-messages`.

## Problem

- **What is in `main`.** `KRN` made the memory and the CPUs for large models: 262-KRN-0160 to 0165, ABI 19 and 20. Nothing serves models yet: there is no `nnd` and no `idl/nn.wit`.
- **What `NRL` has.** On its branch, `nrl-runtime` version 0: a checked `no_std` executor of stateless feed-forward models, with owner- and model-bound sessions, budgets and cooperative cancellation. Its Rust API is linked code, not an IPC interface.
- **What T11 needs.** NRL's T11 (900-NRL-0017) closes only when such a model runs on MIND Core through `nnd`, with:
  - authenticated client identity;
  - budgets and cancellation;
  - two clients kept apart;
  - recovery after a numeric failure.
- **The order.** 262 asks for its stage 0, a design page, before code.

## Plan

1. **The design** ([docs/nn/runtime.md](../docs/nn/runtime.md), 0.1, with this task):
   - what `nnd` holds;
   - `mind:nn@1.0.0`;
   - sessions bound to the kernel-stamped badge;
   - budgets, and what they do not bound;
   - workers, deadlines and cancellation through a control page;
   - the parser session in a worker that holds only the bytes;
   - the native test.

   It is agreed with `NRL` (its questions 1 and 2) and `KRN` (questions 3 and 4) through messages. Their answers change the page into version 1.0.

   **Progress (2026-10-29): the page is at 1.0, agreed.**
   - `KRN` answered questions 3 and 4 (2026-10-28): a line `nnd <MiB>` of `data/memory.conf` gives the quota and the workers; a worker per job until 262-KRN-0179.
   - `NRL` agreed to questions 1 and 2 (2026-10-29): text object files, checked against the writer profile `nrl.serde-json-pretty-object/0`, and f64 only. Its corrections are in: the profile named and narrower than `nrl-lab`'s reader, 16 384 values an object, the whole request checked before it runs, a successful output kept until copied, a row that is not finite refused before it runs, the deadline 0 converted explicitly. It pinned the test model, its references and the encoding vector (900-NRL-0025).
   - `APP` writes the native parser, the encoders and the views (section 6); `NRL` owns no part of it.
   - `KRN` gave how a badge is retired before its reuse (2026-10-29): the shell keeps and revokes the minted capability and sends `nnd` `retire`; `nnd` takes it from badge 0 only (section 1).

2. **`idl/nn.wit` 1.0 and its generated bindings,** as agreed.
3. **`nnd`** (a new directory `nnd/`), linking `nrl-runtime` from `nrl/`:
   - load, with the hash, parser and re-encoding checks;
   - sessions and budgets;
   - one worker per job;
   - status.
4. **The shell:**
   - `REQUEST_NN`: a client badged with a number minted for each start;
   - `nn status | models | load | run | bench`.
5. **The test program `nncheck` and the suite `nnd`** (section 8 of the design), on x86 and aarch64.
6. **Docs:** the design at 1.0; `docs/tools/README.md` and `README_RU.md`; an entry in `docs/profile` for what `nnd` holds and what was shown, on the configurations tested only.

**Progress (2026-10-29): the code, but for the shell and the native run.**
- **Item 2, done:** `idl/nn.wit` 1.0 and its bindings (`libmind/src/idl/nn.rs`), as the design gives it.
- **The checks, in `nnpkg/`** (a `no_std` crate, built for x86 and aarch64):
  - a strict JSON reader;
  - the typed description and its layout between nnd and its workers;
  - the writer profile `nrl.serde-json-pretty-object/0`, with floats through `zmij` 1.0.23 as `NRL`'s writer;
  - the manifest's canonical form and the model ID;
  - `nrl-lab`'s checks ported;
  - `nrl-runtime`'s program with `NRL`'s work policy;
  - the service's state: holds, sessions bound to a badge, budgets checked before a job runs, jobs, cancellation, deadlines, retirement.
- **Item 3, written:** the program `nnd/`, built with the programs for x86 and aarch64:
  - a parser worker per load, holding only the files' bytes;
  - a job worker per job, with the model's sealed description and the job's memory (the cancel flag and the answer);
  - waits deferred;
  - `--memory <MiB>` its quota (asked of `KRN`);
  - a probe worker telling whether it may start workers.

  It runs once 262-KRN-0178 gives it its boot image and grants.
- **Host tests:** 18 in `nnpkg/tests/`, on `NRL`'s pinned bundle, in `scripts/host_tests.sh`:
  - the encoding vector back to its bits and bytes, and 200 000 floats and strings as `serde_json` writes them;
  - every object file in the profile, and the model ID `1b23aaf4…`;
  - `NRL`'s 16 runtime scenarios and their charges;
  - the design's refusals: a wrong ID, a changed byte, another layout, an overlay, an unknown field, a duplicate key, a missing file;
  - the reader against `serde_json`;
  - 20 000 made-up answers of the parser refused without a panic;
  - the badge rules, budgets, cancellation, the deadline, retirement and memory.
- **Left:**
  - item 4, the shell's part (minting, keeping, revoking and retiring a badge; `nn`), which waits for 262-KRN-0178's slot and `REQUEST_NN`;
  - item 5, `nncheck` and the suite;
  - the profile entry, after the native run.

## Acceptance criteria

- [x] **The design is agreed:** `NRL` and `KRN` answer its questions in messages, and the page is at 1.0 (2026-10-29).
- [x] **Host tests** (`nnpkg/tests/`, 2026-10-29):
  - the re-encoding check of a package's description;
  - every bound of the profile;
  - the badge-to-session rule, with every call naming another client's session or job refused;
  - budget charging as `nrl-runtime` defines it.
- **The `nnd` suite, on a named configuration** (QEMU x86 with 4 CPUs; aarch64), steps 1–8 of the design's section 8:
  - loading and its refusals;
  - outputs equal to the reference within `NRL`'s tolerance;
  - the budget, cancellation and the deadline;
  - two clients;
  - a numeric failure and a new session after it;
  - memory back after unload.
- **900-NRL-0017's native criteria** can be ticked from that suite's output. The results name the configuration and are not called a proof (MC-12.1, MC-12.2).

## Related

[262](262-model-runtime-on-the-machines-full-hardware.md); [900](900-neural-representation-lab.md) (NRL), T11; 262-KRN-0160 to 0165 ([0163](../issues-done/262-KRN-0163-workers-on-several-cpus.done): workers); [251](251-model-cache-and-model-disk.md) (the model disk); [178](178-models-on-every-accelerator.md) (later, the accelerators).
