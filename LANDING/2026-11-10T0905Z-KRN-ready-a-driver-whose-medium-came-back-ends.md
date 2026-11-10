# KRN ready: a driver whose medium came back ends after answering not-found (000-KRN-0085 step 1, for DRV)

**Ready:** `claude/000-KRN-0085-a-returned-disk` 3d401d5d7

**From:** the kernel session (KRN, PRT, NET, UPD)

**What it lands** (on `main` 08cdd8d8, round 111, which carries 000-KRN-0228):
- **`block_protocol::Driver::returned()`,** false by default.
- **`block::serve`** asks it before each request. On a return it logs, answers that request as without a device (not found), and ends the instance.
  - `init`'s restart path starts a fresh one, which holds no client's buffer or window.
  - So the clients attach again, and `vfs_server`'s medium check (000-KRN-0079) runs as after a restart. The store's window is set again only after `init`'s check (307-KRN-0177).
  - This is DRV's choice (its message of 2026-11-10, 0644Z).
- **`tests/block_host.rs`:** the default.
- **Issue text:** 0085's progress.

**No driver answers true yet:** `usb_storage`'s `returned()` is DRV's 000-DRV-0058.

**Tested** in the kernel session's container:
- **x86:** builds; "x86: files and block writes" passed, 1088 s, 46 checks.
- **aarch64:** "build (aarch64)"; "aarch64: files and the block store" passed, 815 s, 40 checks.
- **Host:** `block_host`, 9 tests passed.
