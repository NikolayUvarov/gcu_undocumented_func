# KRN ready: 100003-KRN-0291 and 100003-UPD-0052, a reading client of the updater for programs (ABI 39, update.wit 2.1)

**From:** KRN, for `KRN` and `UPD` · **To:** LANDING · **Written:** 2026-11-28 08:42 UTC

**Ready:** `claude/100003-KRN-0291-a-reading-client-of-the-updater` 2a12fc93eb

**Advances:** 100003-KRN-0291 and 100003-UPD-0052, for APP's 100003-APP-0151. Both stay open for the shell's lending, which is APP's part.

**What it is.** APP's request of 2026-11-21: `improve` reads the release the machine runs, and must not be able to update it.
- **ABI 39:** `SLOT_UPDATE_READ` 47, in `LAUNCH_SLOTS`; `SLOT_DYNAMIC` 48, `CAP_SLOTS` 112; `mind::process::REQUEST_UPDATE_READ` is `1 << 31`, the request note's last bit.
- **init** gives the shell a second client of the updater, badged `BADGE_UPDATE_READ`, in slot 47, as for the block store's `BADGE_RECOVER`. The shell's own client keeps no right to badge.
- **The updater** (`updater/src/rights.rs`): every client reads `status`, `metadata` and `kept`; any badge init does not mint, the reading one among them, is `denied` the rest and logged.
- `update.wit` 2.1, its bindings regenerated.

**Tested on its own:** `tests/runtime.rs` pins (48, 39) and the slot list; `tests/updater_host.rs` `the_reading_badge_reads_alone_and_the_gateway_changes_while_remote_update_counts`; the `normal` suite's `update_reading_check` on both architectures ("PASS: the updater's reading client is the shell's, in slot 47, beside its own in slot 35; a program started without REQUEST_UPDATE_READ holds none", in x86's boot group and aarch64's programs group).

**Order:** 262-KRN-0292's request is built on this branch (ABI 40 after 39), so it comes after this one.

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
