# Ready: the updater keeps its releases in the block store, and a rollback rewrites a damaged slot from there (351-UPD-0033)

**From:** STO, making 351-UPD-0033 for the update track · **To:** LANDING · **Written:** 2026-11-09 06:38 UTC · **Concerns:** 351-UPD-0033 (closes), 351-STO-0006 (closes), 351-KRN-0218 (its second criterion shown)

**Ready:** `claude/relaxed-meitner-5bmhpz` 76f0ce826c5245bc96cb3c254a4e55550f6e5220

It is `main` 5bf2fc85 with this branch merged in.

## What it brings

- **The updater keeps releases in the store** (new `updater/src/kept.rs` and `updater/src/store.rs`; `main.rs`, `source.rs`):
  - Each release is a tree of the release layout, with the blobs under `blobs/ab/`. It is pinned through `init`'s client in `SLOT_BLOCKSTORE` (badge `0x17`, 351-KRN-0218), and named by nothing.
  - The running release is kept at start and once a trial is confirmed. A staged release is kept after `fetch` or `install-commit`. A blob a kept release already holds is not put again.
  - After a confirmed boot and after a staging, the store keeps the running release, the last known good and a staged one; the other pins end (`plan::keeps`). Nothing ends on trial.
  - Without a store whose medium outlives a reset, the updater says so at start and runs as before.
- **`update.wit` 1.4** (minor; doc change and regenerated `libmind/src/idl/update.rs`): `rollback` first writes a slot that is not as its manifest lists it again from the last known good in the store, with every check of `fetch`.
- **`blocks check <cid> tree`** reads every file of a tree whole (STO's program).
- **Tests:**
  - `tests/updater_host.rs`: four tests.
  - `tests/qemu_smoke.py`: the `updatestore` suite, run with `updater` and alone. `trial_confirmed` gains `slot="B"`.
- **Docs:** `docs/update/updater.md` 0.9 and `_RU`, `docs/idl`, `docs/storage`, `docs/profile` (Article 9 and the owner bits), `README.md`'s `blocks` line.
- **Issues:** 351-UPD-0033 and 351-STO-0006 moved to `issues-done/`, with the links and both tables.
- **No change** to the ABI, `init` or any service but the updater. `blocks` changes in one command.

## Tested, in this session's container (4 CPUs, QEMU 8.2.2 under TCG)

The tests ran on 0d58f4c2, which was `main` 6eed1563 (with 351-KRN-0218's grant) and this branch. The commits since then change only `issues/`, `issues-done/`, `docs/` and `TRACKS.md`: text, from this branch and from `main`.

- **Built** x86 with `scripts/x86_fixtures.sh`, and aarch64 with `--fixtures`.
- **x86:** `updater`, `linkupdate`, `effector-update` and `store`, 39 checks, all passed. `updater` includes the stub's check with slot 25, `update`, `install` and `updatestore`.
- **aarch64:** the same suites, 40 checks, all passed. `effector-update` and `store` ran again by themselves, after the first run reached its time limit in this session; nothing had failed in it.
- **`updatestore`** on both:
  - release 5 is kept at start;
  - release 6 is fetched and kept, with all but its two changed files taken from release 5's tree;
  - the store logs both pins with `(BADGE 23)`;
  - the shell's `blocks unpin` gets `Rights`;
  - after the apply and the confirmation, both are kept, a collection frees an unpinned object, and both trees check whole;
  - slot A's kernel and loader are damaged on the disk with the machine off. `update rollback` writes release 5 back from the store, and slot A boots on trial with release 5's manifest and is confirmed.
- **`scripts/host_tests.sh`:** every Rust test passed, the four new ones among them. One Python test failed: EFF's `effector_server_test.py` `test_contract_tls_pin_auth_and_prefix`. It is the failure reported to EFF on 2026-11-05, and it fails the same way on `main` in this container. This branch touches nothing of Effector.
- Not run on real hardware.

## A text conflict to expect

`tests/qemu_smoke.py`'s `--suites` help line. This branch adds `updatestore (351-UPD-0033; also in updater)` after `updater`. `fast-test` holds another change to that line (`giveup, 500-ASR-0012`). If that change lands first, keep both. In `fast-test` (f5f5a836) the line is resolved that way.

It is merged into `fast-test` (f5f5a836).
