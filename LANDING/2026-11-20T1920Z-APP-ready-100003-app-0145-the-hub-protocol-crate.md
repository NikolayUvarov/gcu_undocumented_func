# Ready: 100003-APP-0145, MIND Hub's protocol crate `improve/proto`

**Ready:** `claude/wizardly-franklin-kec1a9` 3348e8a47b97f3d6c8021b5f983a98996ff5f4f1

It contains `main` 329a551629. It is in `fast-test` (3a179f55).

- **Closes** 100003-APP-0145, stage 1 of MIND Hub (100003, `docs/hub/README.md` 0.2).
- **What it adds:**
  - the crate `improve/proto` (`no_std` with `alloc`, no `unsafe`), its tests and its vectors;
  - two lines in `scripts/host_tests.sh`: its tests, and its build for `x86_64-unknown-none`;
  - THIRD_PARTY.md names it among the users of `sha2` and `ed25519-dalek`. Nothing new is added: the crates are `apppkg`'s.
- **Nothing that runs on MIND Core uses it yet.** No image changes: no program and no service links it.
- **Tested** in this container:
  - `cargo test`: 11 tests, and the 7 vector files compared;
  - the builds for `x86_64-unknown-none` and `aarch64-unknown-none-softfloat`;
  - the two `host_tests.sh` lines run as the script runs them.
- **Merges** with the waiting request, `LANDING/2026-11-20T1716Z-KRN-ready-801-upd-0046-…` (eed9a278), without a conflict.
