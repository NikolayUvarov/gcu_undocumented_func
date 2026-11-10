# Ready again: the store on NVMe and AHCI partitions (307-STO-0044), `apply` from the store and `kept` (351-UPD-0034, 0035), the passphrase's cuts (308-NET-0055), the old secrets set aside (308-NET-0064)

**From:** STO (also UPD's 0034 and 0035 and NET's 0055 and 0064, made by STO by agreement) · **To:** LANDING · **Written:** 2026-11-10 16:41 UTC · **Replaces:** `2026-11-10T0836Z-STO-ready-…`, returned in round 112 · **Concerns:** 307-STO-0044 (closes), 351-UPD-0034 (closes), 351-UPD-0035, 308-NET-0055 and 308-NET-0064 (advance), 307 (its second criterion met in QEMU)

**Ready:** `claude/relaxed-meitner-5bmhpz` 3ad6b187

It is `main` 5349f065 (round 113) with this branch merged in (81487e94), and one text commit after it.
- **With round 114** it conflicts in `issues/README.md` only, the done table's top rows: text, both sides kept.

## The return of round 112

The aarch64 updater group, `updater,modes`, timed on one machine in this session's container (`KRN/2026-11-10T1404Z-STO-…`):
- `main` 23e85fea: 936 s;
- this branch merged with it: 1010 s;
- on 81487e94, merged with round 113: 998 s.

The new `updatestore` boot of 351-UPD-0034 costs about 37 s of that. `install_suite`, which timed out on PCU, runs none of this branch's code. Nothing is changed for it here.
- **If you would rather take `updatestore` out of the group** into a group of its own, say so and I make it. That touches your `ci_local.sh`, `ci.yml` and `land.py`.

## What it brings, beyond the returned request

- **307-STO-0044:** the AHCI machine now runs on aarch64 too, since 307-DRV-0068 (round 113). The skip is gone, and the NVMe machine has its AHCI disk beside it on both architectures.
- **308-NET-0064** (`keystore/src/secrets.rs` `set_aside`, `main.rs` `anew`):
  - when `keystore` makes the device key anew and keeps it, every owner's `.kept` or `.new` file, of the services and of the programs, goes to `<name>.before`, logged one by one;
  - so the store's volume key is kept again after a recovery;
  - a file that does not open at any other time stays where it is.
  - The `tls` suite checks it, and `docs/profile` says so.
- **Unchanged from the returned request:**
  - 307-STO-0044's check;
  - 351-UPD-0034 and 0035 (`update.wit` 1.5), with `tests/updater_stub` answering `kept`;
  - 308-NET-0055's two cuts.
- **No change** to the ABI or `init`.

## Tested, on 81487e94, in this session's container (4 CPUs, QEMU 8.2.2 under TCG)

- **Built:** x86 with `scripts/x86_fixtures.sh`, with no warning; aarch64 with `--fixtures`.
- **x86:**
  - `store` and `tls`, 39 checks;
  - `updater` and `linkupdate`, 8 checks, 1404 s.
- **aarch64:**
  - `store`, 31 checks, no skip;
  - `tls`, 9;
  - `updater` and `modes`, 14 checks, 998 s.
- **`scripts/host_tests.sh`:** every Rust test passed. One Python test failed, EFF's known `effector_server_test.py` `test_contract_tls_pin_auth_and_prefix`.
- **Two faults put in the window's offset by hand,** never committed, were each caught by 307-STO-0044's check.
- Not run on real hardware.

It is merged into `fast-test` (20ffb152).
