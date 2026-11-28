# KRN ready: 213-KRN-0290, the mirror blends what it shrinks, so no glyph loses a stroke

**From:** KRN · **To:** LANDING · **Written:** 2026-11-28 08:42 UTC

**Ready:** `claude/213-KRN-0290-the-mirror-averages-what-it-shrinks` b02ae9edc7

**Advances:** 213-KRN-0290. It stays open for the Ryzen: the maintainer's eye on both HDMI screens, and the offers' `DRAWN IN` times (MC-12.1).

**What it is.** DRV's report `KRN/2026-11-27T2017Z-…`: the maintainer saw crushed characters on the mirrored screens, because the nearest-pixel shrink skips source rows.
- `mind::display::taps_of` gives each picture pixel its nearest source pixel and the two after it, weighted by coverage, packed in the compositor's tables as before.
- The second and third screens blend up to nine source pixels, each channel apart, in the full draw, the changed spans and under a mark.
- A picture of the first screen's own size is still a copy pixel for pixel.
- **The cost,** measured on the host: a full draw of 2304x1440 from 2560x1600 takes 33 ms where it took 5.6 ms; expect several times the Ryzen's 8.6 ms. The next step, if that is too slow, is stated in the issue.

**Tested on its own:** `tests/display_taps_host.rs`; the display groups below ("x86: boot, display, shell, memory, clock" and the window manager's). QEMU offers no second screen, so they show that the first is unchanged.

**Tested together.** The kernel session's six requests for round 183 were merged with `main` 4df3fe9842 into `claude/KRN-precheck-183`. `main`'s commits since then are text only, merged into each branch afterwards.
- **Host tests here:** 5931 passed, 0 failed (every `tests/*_host.rs` the quick runner builds, and `runtime.rs`); `tests/release_test.py` 19, one skipped as before.
- **On the machines,** each group once. Each later tree adds only the test fixes below.
  - 9b87ca5c92: aarch64 "network, TLS and Effector" (acc1, 1596 s) and "boot and fault containment" (acc2, 177 s); x86 "the updater and an update through the link" (x32-2, 1316 s).
  - 945df29ff2: x86 "boot, display, shell, memory, clock" (x32, 1583 s), "the window manager" (700 s) and "keys, shell, tools" (559 s); aarch64 "programs, shell and four CPUs" (acc2, 472 s) and "tools" (157 s).
  - e730dd468c: x86 "network, TLS and Effector" (acc1, 2070 s).
  - 8e477e9288: x86 "services, storage, audio" (x32, 1172 s) and "scheduling, isolation, heap" (x32-2, 151 s).
  - The gate runs every other group.
- **Four misses of mine, all in test expectations, all fixed on the branches:**
  - the stack check and two other QEMU checks still named the windows' addresses from before 262-KRN-0292 (d4747deb82; it failed aarch64's programs group);
  - the services suite's pmap check, the same (ddf4a12745; it failed x86's services group);
  - the isolation fixture's probes of the guard, the mailbox's edge and the stack, the same (1f5b9f54fd; found by reading, before a run);
  - APP's hub check asserted parse's refusal of a long text "until NET's change", which 100003-NET-0100 is (979ba00e45; it failed x86's network group; APP is told).
- **In `fast-test`** (a9ba59fa6d), all six.
- QEMU's runs say nothing of a real machine (MC-12.1).
