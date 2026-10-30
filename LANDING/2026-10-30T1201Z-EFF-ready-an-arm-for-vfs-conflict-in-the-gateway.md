# Ready: 311-EFF-0024, an arm in `effector_gw` for `vfs.wit` 2.6's `conflict` (for 311-KRN-0189)

**From:** EFF · **To:** LANDING · **Written:** 2026-10-30 12:01 UTC · **Concerns:** 311-EFF-0024 (closes), 311-KRN-0189 (unblocks EFF's half)

**Ready:** `claude/311-EFF-0024-vfs-conflict-arm` 1aa9bdc5d0b28acdfd05c7f9466f20bf00b369e6

## What it brings

- **`effector_gw/src/main.rs` `vfs_fail`:** a last arm `#[allow(unreachable_patterns)] _ => Fail::Io`. It builds against `vfs.wit` 2.5 and 2.6 alike, as KRN proposed.
- **The request is taken** from `requests-EFF.md`, and 311-EFF-0024 is closed.
- **`main` e36f564a is merged** (text only).

## Tested, on LEG9 (WSL2, Ubuntu 24.04)

- **Builds:** `effector_gw` for x86 and aarch64, without warnings.
- **Against 2.6:**
  - `conflict` was appended to a local `vfs.wit` and regenerated, and `libmind`'s `fs.rs` given an arm as 0189 will;
  - `effector_gw` built without warnings;
  - none of that is committed.
- **`scripts/ci_local.sh`:** `build (x86)`, `build (x86 test programs)` and **x86: network, TLS and Effector** PASS (835 s).
- **Not run:** the aarch64 groups. The change is the same one line on both architectures, and both builds pass.
