# 000-KRN-0107 — `02_build.sh` stages `virtio_input` twice; comments that went stale with the 1 ms tick

**Type:** build script and comments (code: through the gate) · **Owner:** kernel session · **Priority:** P3 · **Status:** in progress (made on the branch; the gate left) · **Blocked by:** — · **Main task:** — · **Constitution:** MC-12.3

## Problem

From the critique of 2026-10-14 ([issues-critica-done/02](../issues-critica-done/02_code_and_comliance_analysis.md), §4.1), checked against `main` dfc42ee.

- **`02_build.sh`.** `USER_CRATES` lists `virtio_input:virtio_input:virtio_input.elf` twice, at :69 and :92; the second was added in c6cca01 (issue 161).
  - The build loop skips the repeat through `SEEN`.
  - The staging loop and `scripts/build_aarch64.sh`, which reads the same list, copy the file twice.
  - The copies are the same file, so only time is lost. A list with a repeat invites a second, different line.
- **Comments the 1 ms tick (000-KRN-0083) made wrong.** On x86 the tick is 1 ms, and on aarch64 10 ms. Budgets are still enforced at the end of each 10 ms slice.
  - `libmind/src/time.rs:4`: `sleep` ends "on the first 10 ms tick after them".
  - `libmind/src/time.rs:10`: the monotonic clock falls back to "the 10 ms tick".
  - `common/abi.rs:516`: "Budgets are enforced at the 10 ms tick", where it should say the 10 ms slice.
  - `bench/src/bin/check.rs:156`: "the 10 ms tick only".
  - README:76 and ROADMAP's K3 and C7 lines are text: README's line goes with 000-KRN-0105, and the roadmap's with 000-KRN-0106.
  - `usb_host/src/xhci.rs:53` is the drivers track's ([requests-DRV.md](requests-DRV.md)).

## Plan

1. Delete `02_build.sh`:92.
2. Each comment names the tick of each architecture, or the slice where that is what it means. `grep -rn "10 ms tick"` finds no other line in this track's files, apart from the record in docs/profile/evidence.md, which is dated.

## Acceptance criteria

- [ ] `02_build.sh` and `scripts/build_aarch64.sh` stage each program once, and both builds pass.
- [ ] No comment names a 10 ms tick as x86's.

## Related

000-KRN-0083 (the 1 ms tick). The stale comment in `03_run_qemu.sh`:39 ("audio_gw drives an AC97") goes with [551-DRV-0010](551-DRV-0010-hda-controller-and-codecs.md). `ata`'s "up to 256 sectors" goes to the drivers track ([requests-DRV.md](requests-DRV.md)).
