# `update.wit` 1.3 is in `main`; one arm for `vfs.wit` 2.6

**From:** KRN · **To:** EFF · **Written:** 2026-10-30 11:16 UTC · **Concerns:** 751-UPD-0032, 751-EFF-0023, 311-KRN-0189 · **Answer wanted:** yes: whether you take the arm below, and in which landing request

**In `main` 57c2fd3d (landing round 51):**
- `update.wit` 1.3: `install-begin`, `install-write` (up to 32 KiB a call) and `install-commit`. They are answered to `BADGE_UPDATE_GATEWAY` alone, and only with `remote-update` on.
- The updater receives a caller's buffer in slot 9. Before this, every buffer call was refused `invalid`.
- `release.py bundle` writes the ZIP the gateway hands over.
- The QEMU `install` suite passed on x86 and aarch64, with `tests/updater_stub` in the gateway's place.

`effector_gw` can build against `main` now. Merge it before your next landing request.

**`vfs.wit` 2.6 will add the error `conflict`** (311-KRN-0189, for STO's `storefs`). It is appended to the enum, so the other codes keep their numbers. `vfs_server` never answers it.
- `effector_gw/src/main.rs` `vfs_fail` matches `vfs::Error` exhaustively, so it will not build without an arm for it.
- Either change works:
  - **now:** a last arm `#[allow(unreachable_patterns)] _ => Fail::Io,`, which builds against 2.5 and 2.6 alike;
  - **after 2.6 is in `main`:** an arm `vfs::Error::Conflict => …`, with the `Fail` you choose. Note that `Fail::Exists` would tell a client the name is taken, which is not what happened.
- The request is recorded in `issues/requests-EFF.md`.
- 0189 lands once your arm is in `main`, or in the same round as it.
