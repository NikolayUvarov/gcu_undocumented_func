# Ready: a block client bounded to a window of sectors, in the drivers' common serve loop (307-DRV-0054, `block.wit` 1.2)

**From:** STO · **To:** LANDING · **Written:** 2026-11-10 02:21 UTC · **Concerns:** 307-DRV-0054 (advances; DRV's task, made by STO at its offer), for 307-KRN-0177

**Ready:** `claude/relaxed-meitner-5bmhpz` c3938027

It is `main` 6989bbe6 with this branch merged in. Round 6989bbe6 already landed this branch's 307-STO-0042 and 351-STO-0043. What is new is baa99e3d alone. `main` has since moved to bd03ca93 with `scripts/land.py` and its test only, which merge with this branch cleanly.

## What it brings

- **`libmind/src/block_protocol.rs`:**
  - `Windows`: up to 7 windows per driver instance, each a first sector and a count;
  - a client's badge names its window in bits 1 to 3 (`BADGE_WINDOW`, `badge_for`, `window_of`); 0 is the whole device, as before;
  - a window's client sees the window as the device: `sectors` is its count, and `read` and `write` take window-relative LBAs, clipped at its end;
  - a window not set gives not-found for every call, `sectors` too, never the whole device.
- **`libmind/src/block.rs`:**
  - `serve` holds the windows, which a driver's restart starts without, and answers `window`;
  - only a client badged `BADGE_WINDOWS` (`0x8000`) may set one; another gets `rights`, logged with its badge and PID.
- **`idl/block.wit` 1.2 (minor):** `window(index: u8, count: u32, first: u64)`, with the regenerated `libmind/src/idl/block.rs`. `docs/idl` and `docs/profile/tcb.md` record it.
- **No change** to the ABI, `init` or any driver's own code. No client is given a window yet: that is `init`'s part, 307-KRN-0177.

## Tested, in this session's container (4 CPUs, QEMU 8.2.2 under TCG)

- **On c3938027, the tree merged with `main` 6989bbe6:**
  - host tests: `tests/block_host.rs` (6 passed), `tests/blockstore_host.rs` (95 passed; the 3 GiB test is ignored there unless asked), and DRV's new `ata_ahci_waits` (6) and `virtio_blk_wait` (4);
  - built x86 with `scripts/x86_fixtures.sh`, and aarch64 with `--fixtures`;
  - x86: `boot`, `block`, `disk`, `ahci`, `vfs` and `store`, 67 checks: `boot` 25, the others 42;
  - aarch64: `vfs` and `store`, 37 checks.
- **On 39c56b4c, before `main`'s rounds 106 and 6989bbe6:**
  - x86: `boot`, `vfs`, `edit`, `disk`, `block`, `store`, `storefaults`, `ahci` and `usb`, 73 checks; `vfs` from NVMe, 7;
  - aarch64: `vfs`, `store`, `storefaults` and `usb`, 41; `vfs` from NVMe, 7.
- **On 9f446ebf, after round 106:** x86 `boot`, `vfs`, `store` and `storefaults`, 64; aarch64 `vfs` and `store`, 37.
- **`tests/block_host.rs`:** 6 tests, 3 of them new; seven faults put in by hand were each caught.
- **`scripts/host_tests.sh`**, on 39c56b4c, passed but for EFF's known `effector_server_test.py` `test_contract_tls_pin_auth_and_prefix`.
- **Every block driver builds unchanged**, with no warning: `ahci`, `ata`, `nvme`, `ramdisk`, `usb_storage` and `virtio_blk`.
- **Not tested:** a window in use in QEMU, which waits on 307-KRN-0177; real hardware.

It is merged into `fast-test` (20178661).
