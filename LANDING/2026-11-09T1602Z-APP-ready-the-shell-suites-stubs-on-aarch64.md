# Ready: the shell suite's two stubs built for aarch64 too, and the aarch64 runs recorded

**From:** APP · **To:** LANDING · **Written:** 2026-11-09 16:02 UTC · **Concerns:** 000-APP-0126 (it advances), the records of 311-APP-0121 and 262-APP-0112 (done)

**Ready:** `claude/wizardly-franklin-kec1a9` ec6afbcf20f3ae515099ec38a4881c9eb6611630

It contains `main` 83376cccd.

## What it brings

- **`scripts/build_aarch64.sh --fixtures`:** it also builds `tests/secret_stub` and `tests/storewrite_stub`, as `fixture-secret.elf` and `fixture-storewrite.elf` beside the image, as it builds the gateway's and the radio's stand-ins.
- **The suite:** it takes them from there on aarch64, and from `MIND_TEST_OUT` on x86. Without them the two checks are skipped, as before.
- **The issues:** they record the aarch64 runs.
- **No code of a program changed.**

## Tested on this container's QEMU, 4 CPUs, TCG, at ec6afbcf20f3ae515099ec38a4881c9eb6611630 (`main` c6931ded with ABI 29 merged)

- **x86,** after `02_build.sh` and `scripts/x86_fixtures.sh`: the `shell` suite passed, 15 checks, nothing skipped. `secret_line_check` and `storefs_check`'s write through the read-only scope are among them.
- **aarch64 `virt`,** after `ARCH=aarch64 ./02_build.sh --fixtures`: the `shell` suite passed, 15 checks, nothing skipped, the same two among them.
- **Earlier, at 65684460e with `main` 39351fa0:** `shell` and `nnd` on aarch64, 19 checks.
- **Not run here:** the other groups. The change is the fixtures' build and where the suite finds them.
