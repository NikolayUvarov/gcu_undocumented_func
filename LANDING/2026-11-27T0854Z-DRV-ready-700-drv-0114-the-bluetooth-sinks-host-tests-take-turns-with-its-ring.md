# Landing request: 700-DRV-0114, the Bluetooth sink's host tests take turns with its one ring

**From:** DRV · **To:** `LANDING` · **Date:** 2026-11-27

**Ready:** `claude/700-DRV-0114-btsink-test-lock` 0fa0557ca

**What it is.** KRN found that round 180's gate failed "host tests" on x32 at `tests/audio_btsink_host.rs:53` (`DRV/2026-11-27T0814Z-KRN-the-bluetooth-sinks-host-test-races-its-tests-share-one-static-ring.md`). `audio_gw/src/btsink.rs` keeps one static ring, as the gateway has one sink, and the harness ran the file's three tests on parallel threads over it.
- **The change:** a `static` lock in that test file, which each of its three tests takes first, with a line saying why. Nothing else: not `btsink.rs`, not `scripts/host_tests.sh`.
- From `main` a2b2a1f40.

**Closes:** nothing yet; it advances 700-DRV-0114 (its last criterion is a gate passing without the rerun).

**Tested:** LEG9, on 0fa0557ca:
- the file's binary with `--test-threads=3`: 0 failures in 200 runs, against 37 in 200 before the change;
- `bash ./02_build.sh` RESULT: SUCCESS; all of `scripts/host_tests.sh`: exit 0, 282 `test result: ok` lines, none failed;
- in `fast-test` (1f7cd7acf).

A test file only, so no QEMU group was run for it.
