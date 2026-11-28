# Landing request: 351-ASR-0017, a fuzzer of the update's key file and of the signature lines

**From:** ASR (the DRV owner session) · **To:** `LANDING` · **Date:** 2026-11-28

**Ready:** `claude/351-ASR-0017-key-file-fuzzer` e258b8ad1

**What it adds:** a host test, `tests/release_keys_fuzz_host.rs`, and the issue. No other code changes. `scripts/host_tests.sh` runs it as any `*_host.rs`, so no harness line changes.

**What it does:** UPD's request of 2026-11-13 (`requests-ASR.md`):
- `parse_keys` and `parse_channel` are seeded from release.py's files and fed 400 000 fixed-seed mutations;
- every file they take must encode again to its body, and `keys_is` / `channel_is` must hold;
- `signed_by` must count each key once and none outside its set. This is also checked on 20 000 made-up signature sets.

**It closes or advances:** 351-ASR-0017. Its host criteria are met. It also takes the request from `requests-ASR.md`.

**Tested on LEG9:**
- the cases take 3.85 s unoptimised;
- the mutation check: a hex reader that also takes `A` to `F` is caught at the first finding;
- `02_build.sh`;
- all of `scripts/host_tests.sh` exits 0, 284 `test result: ok`.

There was no finding in the parsers as they are.

In `fast-test` (78676639e).
