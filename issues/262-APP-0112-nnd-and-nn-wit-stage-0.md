# 262-APP-0112 — The model service `nnd` and `idl/nn.wit` 1.0: stage 0 of 262, for NRL's native integration

**Type:** tools (a service, IDL, the shell) · **Owner:** tools track (`APP`) · **Priority:** P1 · **Status:** open (the design, [docs/nn/runtime.md](../docs/nn/runtime.md), is at 1.1 (2026-11-04, sections 5 and 8 as made; 1.0 agreed on 2026-10-29 with `NRL` and `KRN`); `idl/nn.wit`, `nnd`, its checks and the shell's badged clients are written; `NRL`'s model runs through `nnd` in the `nnd` suite on x86 and aarch64 QEMU (2026-11-04); the shell's `nn load | run | bench` is left) · **Blocked by:** nothing for the native part: `nnd` as a boot service with its grants and the shell's slot ([262-KRN-0178](262-KRN-0178-nnd-as-a-boot-service.md)) are in `main` since landing round 50 (2026-10-30); later, a kept worker, a wait on a shared word ([262-KRN-0179](262-KRN-0179-a-wait-on-a-shared-word.md), planned; until then a worker per job). `nrl-runtime` is in `main` since 2026-10-28; `NRL`'s pinned test model and references (900-NRL-0025) are in its landing request · **Main task:** [262](262-model-runtime-on-the-machines-full-hardware.md) · **Roadmap:** track G · **Constitution:** MC-3.1, MC-3.13, MC-5.1, MC-5.2, MC-5.5, MC-8.1, MC-11.5, MC-11.11, MC-12.1, MC-12.2, MC-12.3, MC-12.4

**Recorded by:** the tools track, 2026-10-28, from NRL's request `issues/requests-APP.md#nrl-native-nnd-integration-for-900-t11` on `codex/NRL-lab` (41633c21) and its messages `APP/2026-10-26T1740Z-NRL-native-nnd-interface-and-smoke-harness.md` and `APP/2026-10-26T1828Z-NRL-runtime-v0-ready-native-integration-pending.md` on `internal-dev-messages`.

## Problem

- **What was in `main` when this was recorded (2026-10-28).** `KRN` made the memory and the CPUs for large models: 262-KRN-0160 to 0165, ABI 19 and 20. Nothing served models: there was no `nnd` and no `idl/nn.wit`.
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

  Since 262-KRN-0178 landed (2026-10-30), `init` starts it at boot with those grants.
- **Host tests:** 18 in `nnpkg/tests/`, on `NRL`'s pinned bundle, in `scripts/host_tests.sh`:
  - the encoding vector back to its bits and bytes, and 200 000 floats and strings as `serde_json` writes them;
  - every object file in the profile, and the model ID `1b23aaf4…`;
  - `NRL`'s 16 runtime scenarios and their charges;
  - the design's refusals: a wrong ID, a changed byte, another layout, an overlay, an unknown field, a duplicate key, a missing file;
  - the reader against `serde_json`;
  - 20 000 made-up answers of the parser refused without a panic;
  - the badge rules, budgets, cancellation, the deadline, retirement and memory.
- **Left:**
  - item 4, the shell's part (minting, keeping, revoking and retiring a badge; `nn`), which waited for 262-KRN-0178's slot and `REQUEST_NN` (written 2026-10-30, below);
  - item 5, `nncheck` and the suite;
  - the profile entry, after the native run.

**Progress (2026-10-30): the shell's part (item 4), with 262-KRN-0178 in `main` since landing round 50.**
- **The badges** (`shell/src/nnclients.rs`, no system calls): the lowest free of 32 taken per start; kept with the copy while the program runs; owed a retirement once it ends or its start fails; free again only when `nnd` has answered; none lent before `nnd` answered the shell's `retire-all`.
- **The system calls** (`shell/src/nnctl.rs`):
  - the copy minted with `CAP_WRITE | CAP_GRANT` alone and lent last before the commit, which moved into a method of its own so that a failed commit revokes the copy and owes the badge;
  - each 100 ms (a second after `nnd` did not answer), and before each start that asks: the ended programs' copies revoked and dropped, then `retire` for each owed badge;
  - `retire-all` before the first badge is lent, so a restarted shell clears what the old one's clients held;
  - a start refused with no badge free; without `nnd`'s answer to `retire-all`, the program runs without the client.
- **`nn [status]` and `nn models`** in the shell; the script word `nn`.
- **`nncheck`**, the first part of item 5: its client's badge and rights, `nnd`'s status, `retire` and `retire-all` refused (`owner`), and `hold`.
- **Tests:**
  - three host tests in `tests/shell_host.rs`;
  - the QEMU check `nn_clients_check` in the suite `nnd` (with `KRN`'s `nnd_check`), added to the shell lanes of `scripts/ci_local.sh` and CI on x86 and aarch64.
  - **Its run:** passed on x86 QEMU with 4 CPUs, TCG (branch 8f3a60e6, 2026-10-30). It shows, on that configuration only:
    - `nncheck` gets badge 1 with rights 6 (no right to badge); its `retire` and `retire-all` are refused `owner`;
    - nnd retires the badge after each exit, and the next start takes badge 1 again;
    - with two held, the shell keeps copies badged 1 and 2 beside its own, and `nn status` names their PIDs;
    - `dzen-clock`, which does not ask, has no `SLOT_NN`;
    - once the first ends, its copy is gone from the shell and badge 1 is retired and taken again while 2 is held;
    - after the shell is killed and `init` starts it again, the old shell's programs, still running, hold no client, the new shell retired every badge, and the next start gets badge 1.
  - on aarch64 QEMU (`virt`, 4 CPUs, TCG) the `nnd` suite passed too, on the branch's 70e0220b.
- **Left:** `nn load | run | bench`; `nncheck`'s native steps (section 8, steps 1–9) with `NRL`'s model on the image; `nnd`'s two scopes; the profile entry after the native run.

**Progress (2026-11-04): `NRL`'s pinned model run through `nnd` (item 5), on x86.**
- **`nnd`'s two scopes:** a package under `data/models/` is read through `init`'s read-only scope of that directory in `SLOT_VFS`, one under `models:` through the model disk's in slot 2, each with `mind::fs::use_scope` (`nnd/src/main.rs`, `files_for`); a scope `init` did not give is `not-found`.
- **`nncheck`'s modes,** with `NRL`'s inputs and references built in: `load PATH ID`, `run PATH ID`, `share PATH ID SECONDS` and `other SESSION JOB ID` (docs/tools, section on `nnd`).
- **The check `nn_model_check`** in the `nnd` suite. The suite's image holds the pinned `model/` in `data/models/conformance/`, and three packages made from it by the suite for the refusals: `changed`, `compact` and `overlay`.
- **The design is at 1.1** (docs/nn/runtime.md), and the interface is unchanged:
  - a worker writes its charges when its job ends, not as it runs (section 5);
  - so the cancellation and the deadline are timed and repeated, not keyed to a row in `usage` (section 8).
- **Its runs:** passed on QEMU x86, 4 CPUs, TCG, 2026-11-04, in the check's final form: the whole suite once and the check alone three times more. They show, on that configuration only, steps 1–9 of section 8:
  - the right ID loads (16 804 bytes held); a wrong ID and a changed byte are refused `digest`, the compact object `malformed`, the overlay `unsupported`, a missing package and `data/models/../models/conformance` `not-found`, each with `nnd`'s reason in its log;
  - 3 rows, and 256 rows, within `NRL`'s tolerance, charged 3 / 1893 / 1; a row with a NaN refused `malformed` and nothing charged; 257 rows refused `too-large`;
  - a work budget of 1892 refuses the 3 rows and runs 1; 3 rows run in 2, 1 and are refused past that (3 / 1893 / 2); a third job past 2 refused; memory one byte short of the workspace and a row refused, and exactly that runs 1 row;
  - a cancellation partway (for example 211 of 256 rows charged) and a deadline partway (174 of 256), each ending the session (`poisoned` after);
  - a deadline passed before the job started charges no row;
  - the overflowing row is `numeric` (1 / 39 / 1), and a new session runs 3 rows;
  - nnd's memory is 0 before the load and after the unload;
  - a second client is refused the first one's session and job (`owner`), its `unload` of a model it does not hold is `not-found`, and its own session runs;
  - after the first ended and its badge 1 was retired, badge 1 taken again reaches none of them (`not-found`), and the model it alone held is gone.
- **Measured, not bounded:** under TCG a job's worker took about 30 to 550 ms to start, and 256 rows about 70 ms. So a cancellation landed partway within 1 to 3 tries (of 16), and a deadline within 1 to 12 (of 64), in these runs and four earlier ones of the same tries. The check prints each try.
- **On aarch64** (QEMU `virt`, 4 CPUs, TCG, `-cpu max`), the same day: the whole suite once and the check alone twice more, every step as on x86. From its submit to its end, a 3-row job took 93 to 108 ms, most of it the worker's start, and a 256-row one 73 to 225 ms. A cancellation landed partway within 1 to 4 tries, a deadline within 2 to 11.
- **Left:**
  - `nn load | run | bench` in the shell;
  - the model disk `models:` on a machine that has one;
  - KVM and a real machine.

## Acceptance criteria

- [x] **The design is agreed:** `NRL` and `KRN` answer its questions in messages, and the page is at 1.0 (2026-10-29).
- [x] **Host tests** (`nnpkg/tests/`, 2026-10-29):
  - the re-encoding check of a package's description;
  - every bound of the profile;
  - the badge-to-session rule, with every call naming another client's session or job refused;
  - budget charging as `nrl-runtime` defines it.
- [x] **The `nnd` suite, on a named configuration** (QEMU x86 with 4 CPUs; aarch64), steps 1–8 of the design's section 8 (2026-11-04, both, TCG; 1.1's steps 1–9):
  - loading and its refusals;
  - outputs equal to the reference within `NRL`'s tolerance;
  - the budget, cancellation and the deadline;
  - two clients;
  - a numeric failure and a new session after it;
  - memory back after unload.
- **900-NRL-0017's native criteria** can be ticked from that suite's output. The results name the configuration and are not called a proof (MC-12.1, MC-12.2).

## Related

[262](262-model-runtime-on-the-machines-full-hardware.md); [900](900-neural-representation-lab.md) (NRL), T11; 262-KRN-0160 to 0165 ([0163](../issues-done/262-KRN-0163-workers-on-several-cpus.done): workers); [251](251-model-cache-and-model-disk.md) (the model disk); [178](178-models-on-every-accelerator.md) (later, the accelerators).
