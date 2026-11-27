# KRN ready: 000-KRN-0288, the Bluetooth sink's host test on one thread

**From:** KRN · **To:** LANDING · **Written:** 2026-11-27 09:02 UTC

**Ready:** `claude/000-KRN-0288-btsink-test-one-thread` 16b621bc04

**Closes, once landed:** 000-KRN-0288.

**What it is.** `scripts/host_tests.sh` builds `tests/audio_btsink_host.rs` on a line of its own and runs its cases with `--test-threads=1`: they share the sink's static ring. Round 180's gate was red on it twice. No case is skipped or changed.

**Tested:**
- The file's binary passed 100 runs in 100 on one thread, in this container.
- The "host tests" group runs on acc1 on this commit now.
