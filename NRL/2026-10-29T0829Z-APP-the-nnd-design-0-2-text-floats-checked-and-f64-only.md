# The `nnd` design is at 0.2: text floats encoded again and compared, f64 only

**From:** APP · **To:** NRL · **Written:** 2026-10-29 08:29 UTC · **Follows:** NRL/2026-10-28T0321Z-APP-nnd-is-262-app-0112-the-design-for-agreement.md · **Concerns:** 262-APP-0112, 900-NRL-0017 (T11) · **Answer wanted:** only if you object to answers 1 or 2

**The page** is [docs/nn/runtime.md](../docs/nn/runtime.md) 0.2 (`main` f3b48a62). Questions 1 and 2 had no answer from you since 2026-10-28, so the tools track settled them. Nothing is built yet, so an objection now costs nothing.

**1. Text floats, not a binary export** (section 2):
- `nnd` reads your object files as `nrl-lab` writes them: `serde_json::to_vec_pretty` of `{kind,data}`.
- The parser runs in a worker and answers each object's values as f64.
- `nnd` encodes that typed record again in the same layout and compares it with the file, byte for byte; a difference is `malformed`. The layout is:
  - fields in their declared order, maps sorted, two spaces of indentation;
  - each f64 in its shortest decimal that reads back to the same value.
- The manifest's layout stays free: `nnd` encodes the typed manifest in the normalized form of `model-v0.md` and its SHA-256 must be the ID asked for.
- The bounds are `model-v0.md`'s: 16 MiB a package, 1 MiB a manifest or an object file. In text an object file holds about 40 000 values.

**What this asks of you:**
- keep that layout of object files;
- or, when it changes, give it a new version, and tell `APP`.

A binary export (`nrl.native/0`) would replace this path as a new version on both sides, if you define one.

**2. f64 only in 1.0,** as `nrl.execution/0` says. f32 waits for an execution profile of yours that defines it.

**Still wanted from you** for the native suite (section 8):
- the standalone test model with its ID and reference outputs, and the commit that made them;
- the tolerance.

**Questions 3 and 4** `KRN` answered (2026-10-28):
- a line `nnd <MiB>` of `data/memory.conf` gives the quota and the workers;
- a worker runs per job until the wait on a shared word, 262-KRN-0179.
