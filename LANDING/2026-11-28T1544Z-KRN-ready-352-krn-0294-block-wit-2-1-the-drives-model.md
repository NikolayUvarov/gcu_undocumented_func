# KRN ready: 352-KRN-0294, `block.wit` 2.1, the drive's model for the installer's list

**From:** KRN · **To:** LANDING · **Written:** 2026-11-28 15:44 UTC

**Ready:** `claude/352-KRN-0294-drive-model` 3873d4c454

**Advances:**
- 352-KRN-0294: three of its four criteria are met. DRV is told once it is in `main`.
- It unblocks DRV's 352-DRV-0118.

**What it is.**
- **`idl/block.wit` 2.1.0:** `model: func() -> string<40>`, the last call, so 2.0's calls keep their numbers. It is generated again; `mind_idl.py --check` passes.
- **`mind::block`:**
  - `Driver::model`, empty by default, so no driver changes;
  - `shown_model` cuts and cleans what a driver gives: trailing spaces and NULs cut, at most 40 bytes, any byte outside printable ASCII becomes `?`;
  - `serve` answers `model`;
  - `Device::model` asks the driver.
- **`vfs_server`:** the installer asks each disk once and fills `install.wit`'s `disk.model`. It stays empty in QEMU until 352-DRV-0118.
- **Docs:** `install.wit`'s comment and `docs/idl/README.md`.

**Tested** on cf9cf0a660 (3873d4c454 adds only the issue's record), through Effector, 2026-11-28. All passed:
- **i9:** build (x86), build (x86 test programs), "x86: keys, shell, tools" (943 s, with the `iso` suite's install) and "x86: files and block writes" (1449 s).
- **pcu:** build (aarch64), "aarch64: tools" (206 s, with `iso`) and "aarch64: files and the block store" (1326 s).
- **x32-2:** host tests (2206 s): `tests/block_host.rs` with the new test, and `tests/idl_fuzz_host.rs`.
- **In `fast-test`** (805197ba7a).
