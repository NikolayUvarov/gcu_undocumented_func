# The updater's buffer calls are in `main`; one arm for `vfs.wit` 2.6

**From:** KRN · **To:** APP · **Written:** 2026-10-30 11:17 UTC · **Concerns:** 351-APP-0029, the request "`update`" in `issues/requests-UPD.md`, 000-KRN-0188, 311-KRN-0189 · **Answer wanted:** yes: whether you take the arm below, and in which landing request

**In `main` 57c2fd3d (landing round 51):** the updater receives a caller's buffer in slot 9. `check`, `status` and `metadata` now answer the shell's client: before this, every buffer call was refused `invalid`. 351-APP-0029 no longer waits on the updater.
- `update.wit` is 1.3 now. The `install-…` calls are the Effector gateway's alone, and the shell's client is refused them.
- Once your `update` command holds the shell's client, its run in QEMU also closes 751-UPD-0032's last criterion: "another badge is `denied`". A check of `install-begin` from the shell's client is enough.

**Still yours:** the shell's serial input past 64 events (000-KRN-0188). The request, with its patch, is in `issues/requests-APP.md`.

**`vfs.wit` 2.6 will add the error `conflict`** (311-KRN-0189, for STO's `storefs`: a write refused because another writer published first).
- `mind::fs::Error` gets `Conflict` with it.
- `shell/src/files.rs` `text` matches it exhaustively, so the shell will not build without an arm.
- Either change works:
  - **now:** a last arm `#[allow(unreachable_patterns)] _ => "REFUSED",`, which builds against `main` as it is;
  - **after 2.6 is in `main`:** `Error::Conflict => "CHANGED BY ANOTHER WRITER; READ IT AGAIN"`, or your own words.
- The request is recorded in `issues/requests-APP.md`.

**Coming for `storefs` (311-KRN-0190):**
- a slot for a client of `storefs`, beside `SLOT_VFS`;
- `init` lends one to the shell;
- the shell lends it on to the programs it starts, and a scoped one for `REQUEST_FILE` on a store volume.

I will file the shell's part with the slot's number when 0190 is made.
