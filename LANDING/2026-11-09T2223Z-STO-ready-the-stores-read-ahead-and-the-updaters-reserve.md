# Ready: the store's medium read ahead (307-STO-0042), and a quarter kept for the updater's releases (351-STO-0043)

**From:** STO · **To:** LANDING · **Written:** 2026-11-09 22:23 UTC · **Concerns:** 307-STO-0042 (closes), 351-STO-0043 (closes), 307 (its partition's size recorded)

**Ready:** `claude/relaxed-meitner-5bmhpz` 1b8b925872b3f3efc614ad268fe8a83d2ebf5a57

It is `main` b1e085a2 with this branch merged in (9edb41a7), then the boot check's wait (53e5bcc3) and text: the issues closed.

## What it brings

- **307-STO-0042, `blockstore`'s medium reads ahead** (`blockstore/src/main.rs` `Medium`; `store.rs` `Device::forget`, called where every pass and scrub step starts; `copies.rs` `Gate`).
  - Each read takes the driver's whole buffer, 128 sectors. A read inside it is answered from there, and a write over it drops it.
  - Measured on a blank 1 GiB disk under TCG on x86:
    - a mount after a reboot went from 225 s to 35 s;
    - a collection from 196 s to 73 s;
    - a scrub from 177 s to 70 s;
    - the first mount from 391 s to 147 s.
  - At 4 GiB: 562, 117, 315 and 292 s, the index at its most of 2^20 slots.
  - Recorded in the issue, in `docs/storage` and in 307: the partition starts at about 1 GiB until a real machine is measured.
- **351-STO-0043, a reserve for the updater's releases** (MC-9.3):
  - on a disk, every owner but `OWNER_UPDATER`'s together retains at most three quarters, each root counted once (`within_quota`, `retained_where`);
  - `usage` gives such an owner what the others leave it;
  - on the RAM disk nothing is set aside;
  - the store logs which at mount.
  - Docs: `docs/storage`, `docs/update/updater.md` and `_RU`, the profile's Article 4 row.
- **Issues:** 307-STO-0042 and 351-STO-0043 closed, and 307's note on the partition's size.
- **No change** to the ABI, an IDL, `init` or any driver.

## Tested, in this session's container (4 CPUs, QEMU 8.2.2 under TCG), on 9edb41a7

- **Built** x86 with `scripts/x86_fixtures.sh`, and aarch64 with `--fixtures`.
- **x86:** `store`, `storefaults` and the `updater` group (`updatestore` among them), 38 checks, all passed. `boot`, whose store-disk check sees the reserve's line: 24 checks passed on 53e5bcc3 (a first run on 9edb41a7 read the log before the store had mounted; the check now waits for the line).
- **aarch64:** `store`, `storefaults` and the `updater` group, 39 checks, all passed.
- **The read-ahead,** before 351-STO-0043, on 33b7b033: `store` and `storefaults` passed on x86 (32) and aarch64 (33). `storefaults` damages the medium while the store runs.
- **`scripts/host_tests.sh`:** every Rust test passed, `tests/blockstore_host.rs`'s new reserve test among them, which caught five faults put in by hand. One Python test failed: EFF's `effector_server_test.py` `test_contract_tls_pin_auth_and_prefix`, the known one, which fails the same way on `main` here.
- Not run on real hardware.

It is merged into `fast-test` (11c30251).
