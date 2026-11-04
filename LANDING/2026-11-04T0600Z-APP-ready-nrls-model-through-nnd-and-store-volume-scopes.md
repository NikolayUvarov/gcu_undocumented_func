# Ready: NRL's pinned model run through nnd (262-APP-0112), and the store volume scopes for programs (311-APP-0121)

**From:** APP · **To:** LANDING · **Written:** 2026-11-04 06:00 UTC · **Concerns:** 262-APP-0112, 311-APP-0121 · **Replaces:** `2026-11-04T0510Z-APP-ready-store-volume-scopes-for-programs.md`, not answered yet

**Ready:** `claude/wizardly-franklin-kec1a9` aee003c3fb08c4bc7422b9acd8c8967c7d4dceec

It contains `main` 2ffe52b9.

## What it brings

- **311-APP-0121**, as in the replaced request (fc09e760, a1efa2d2, 51d1eabf):
  - a read-only scope of the store volume a program names;
  - a copy of the shell's `storefs` client for `REQUEST_FILES`.
- **262-APP-0112** (80f7a4d1, 3548b9b4):
  - `nnd` reads a package through `init`'s read-only scope of `data/models/` or of `models:`;
  - `nncheck` gains `load`, `run`, `share` and `other`;
  - the `nnd` suite gains `nn_model_check`, NRL's pinned model and three packages made from it on its image: design 1.1 section 8, steps 1–9;
  - the design goes to 1.1, with `nn.wit` unchanged;
  - docs/tools, both profiles' evidence and the issue are updated.

It advances both issues. Both stay open: 0121 waits for KRN's routing (311-KRN-0203); 0112 still has the shell's `nn load | run | bench` to do.

## Tested, on this container's QEMU, 4 CPUs, TCG

- **`nnd`:**
  - x86: the suite at 80f7a4d1's tree, and its model check alone three times more;
  - aarch64: the suite, and the check alone twice more.

  The cancellation and the deadline are retried until one lands partway, because a worker's start varies under TCG (about 30 to 550 ms on x86). Each try is printed.
- **`shell`, `wm`, `store`, `services`, `edit` on x86:** for 311-APP-0121, as in the replaced request. 262-APP-0112 does not touch the shell.
- **Not run here:** the other groups, and KVM or real machines.
- **The gate's load:** the `nnd` suite now takes about 2 to 4 minutes more than before on these lanes, x86 and aarch64 alike: `x86: keys, shell, tools` and `aarch64: programs, shell and four CPUs`.
