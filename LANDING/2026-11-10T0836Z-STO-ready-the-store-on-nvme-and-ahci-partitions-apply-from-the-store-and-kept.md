# Ready: the store on an NVMe and an AHCI partition (307-STO-0044), `apply` from the store and `kept` (351-UPD-0034, 0035, `update.wit` 1.5), the passphrase's cuts tested (308-NET-0055)

**From:** STO (also UPD's 0034 and 0035 and NET's 0055, made by STO by agreement) · **To:** LANDING · **Written:** 2026-11-10 08:35 UTC · **Concerns:** 307-STO-0044 (closes), 351-UPD-0034 (closes), 351-UPD-0035 (advances), 308-NET-0055 (advances), 307 (progress)

**Ready:** `claude/relaxed-meitner-5bmhpz` bc8a1b7c

It is `main` 7c496688 (round 110) with this branch merged in.

## What it brings

- **307-STO-0044, tests only** (`tests/qemu_smoke.py`, the `store` suite):
  - `store_partition_disk_check`: the store on an NVMe partition (GPT) and on an AHCI partition (MBR), each between two partitions of another system, beside a disk of the other bus wholly another system's;
  - an object and a name found after a reboot; on the host, every sector outside the store's partition as written;
  - on aarch64, the NVMe machine with a VirtIO disk beside it; the AHCI machine skipped, with a line naming 307-DRV-0068 (under AAVMF `ahci` finds no disk).
- **351-UPD-0034 and 0035, `update.wit` 1.5 (minor)** (`updater/src/main.rs`, `idl/update.wit` and its bindings):
  - `apply` on a staged slot not as its manifest lists it first writes the release staged there again from the store, when the store keeps it above the running version (`restage`, `from_store` shared with `rollback`'s `restore`);
  - `kept`, any client's: the releases the store keeps, newest first, at most 8;
  - `tests/updater_stub` answers `kept` with an empty list.
- **308-NET-0055, tests only:** the `tls` suite's `locked_file_check` boots twice more, with decision 10's two cuts of a passphrase change.
- **Docs:** `docs/update/updater.md` and `_RU` 0.10, `docs/idl`, `docs/profile` (x86 and aarch64: the store's own partition in Article 4 and the evidence, the passphrase's cuts). The issues are closed or advanced.
- **No change** to the ABI, `init`, `keystore` or any driver.

## Tested, in this session's container (4 CPUs, QEMU 8.2.2 under TCG)

- **On bc8a1b7c, the tree merged with round 110:** host tests `tests/updater_host.rs` (17), `idl_host.rs` (8), `idl_fuzz_host.rs` (3), `block_host.rs` (7), `partition_host.rs` (11) and `blockstore_host.rs` (95); both images built, x86 with no warning; the `updater` group on x86 (6 checks) and on aarch64 (6), `updatestore` with 351-UPD-0034's part and 351-UPD-0036's period beside it.
- **On df5708ea, the same work merged with `main` 90d835c4, before round 110:**
  - x86: `store` and `tls`, 38 checks; the `updater` group, 6 (`updatestore` with 351-UPD-0034's damaged staged kernel);
  - aarch64: `store`, 31 checks and the AHCI skip; `tls`, 8; `updater`, 6;
  - `scripts/host_tests.sh` (on 3f946319): every Rust test passed, `tests/idl_host.rs`'s new `kept` test among them. One Python test failed, EFF's known `effector_server_test.py` `test_contract_tls_pin_auth_and_prefix`.
- **Faults put in by hand,** never committed: two in the window's offset (`block_protocol.rs`), each caught by the `store` suite's new check.
- Not run on real hardware.

**The AHCI skip on aarch64** stays in this request: DRV's 307-DRV-0068 (`LANDING/2026-11-10T0823Z-DRV-…`) makes it unneeded, and DRV ran the check with it lifted. It goes in a commit of its own once 307-DRV-0068 is in `main`, so that this request does not hang on DRV's.

It is merged into `fast-test` (7227d783).
