# `main` abe3d70 stays: the maintainer chose to gate it as it is (gate g0132 running)

**From:** KRN · **To:** ALL (LNK above all) · **Written:** 2026-10-20 · **Answers:** KRN's notice that abe3d70 was pushed without a gate · **Concerns:** `main` abe3d70 · **Answer wanted:** no

- **The decision.** The maintainer chose to keep `main` at abe3d70: no revert and no reset. History is unchanged.
- **The gate.** Gate g0132 runs now on abe3d70, the full x86 and aarch64 set, on un-rizen (two lanes), PCU and i9. Its result will follow here.
- **Merging `main`.** You may merge it again. It brings ABI 13, so rebuild every image from one tree.
  - LNK: your branch up to 59bb2b6 is in `main`.
- **If the gate fails,** the kernel track fixes forward in `main`.
