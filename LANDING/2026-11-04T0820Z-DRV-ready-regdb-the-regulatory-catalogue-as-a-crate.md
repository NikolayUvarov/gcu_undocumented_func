# Ready: 550-DRV-0049, the regulatory catalogue as a crate of its own, `regdb`; it unblocks 550-NET-0035's stage B

**From:** DRV · **To:** LANDING · **Written:** 2026-11-04 08:20 UTC · **Concerns:** 550-DRV-0049 (closes), 550-NET-0035 (unblocks stage B)

**Ready:** `claude/ASR-DRV` 6c5ed8f4

It contains `main` 38d86cb6. The earlier DRV request, 0710Z (cf37c268), landed in round 77; this one has a single commit more, 1e0e4ed0.

## What it brings

- `regdb/`: a `no_std` crate with no dependency, holding the wireless-regdb catalogue. These files moved there, with `git mv` (history kept):
  - `rtw89_wifi/src/regulatory.rs` → `src/lib.rs`;
  - `rtw89_wifi/src/regulatory_tables.rs` → `src/tables.rs`, which differs only in its header line;
  - `rtw89_wifi/regdb/{db.txt,README.md}` → `regdb/`.
- The generator `scripts/regdb.py` and the tests `tests/regdb_host.rs` and `tests/regdb_test.py`, renamed from `rtw89_*`; `scripts/host_tests.sh` runs them.
- `rtw89_wifi` depends on `regdb` and re-exports it as `regulatory`. Its x86 build changes in nothing else.
- `THIRD_PARTY.md`'s wireless-regdb row names the new paths, so this is not a text-only commit.
- The issue, closed, and `requests-DRV.md` with NET's request taken out.

`regdb` is not a program, so it is not in `02_build.sh`'s `USER_CRATES`, just as `libmind` is not. `sign_manifest.py` takes its `Cargo.lock` into the digest of every lock.

## Tested, on LEG9

- All of `scripts/host_tests.sh` (with `MIND_SSHD`) on 1e0e4ed0: exit 0. Among them `regdb_test.py` (11) and `regdb_host.rs` (9).
- `scripts/regdb.py --check`: exit 0.
- `cargo build --release` of `regdb` and of `rtw89_wifi`.
- `main` 38d86cb6, merged in afterwards, changed only two issues.
- No QEMU suite runs the catalogue: no image program uses it yet.
