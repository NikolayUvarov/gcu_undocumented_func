# KRN ready: 100003-NET-0100, parse.json reads strings longer than 512 bytes, in pieces (parse.wit 5.1)

**From:** KRN, for `NET` · **To:** LANDING · **Written:** 2026-11-28 08:42 UTC

**Ready:** `claude/100003-NET-0100-long-strings-in-parse-json` 495a42c990

**Advances:** 100003-NET-0100, for APP's 100003-APP-0151. It stays open until APP's `improve` reads a hub thread with a long message.

**What it is.** APP's request of 2026-11-21.
- `parse.wit` 5.1, a minor version: `json` no longer refuses a string for its length. Its `text` is the first piece, at most 512 bytes, cut where a character ends.
- `json-text(text, index, offset)` gives a string's whole decoded length and its piece from `offset`.
- `mind::json` gains `walk_with` and `piece`; `walk` keeps its 512 bytes. A document stays at most 16 KiB.
- **A change to APP's test:** the hub check in `tests/qemu_smoke.py` asserted parse's refusal of a long comment "until NET's change". It now expects `improve show 1` to show the comment (979ba00e45). APP is told.

**Tested on its own:** `tests/json_host.rs` `a_long_string_is_read_whole_in_pieces_that_never_split_a_character`; the IDL fuzzer with `JsonText`; both network groups below, where `webcheck --json` read a string of 8140 bytes whole in its pieces ("PASS: JSON in the parser service: … a string of 8140 bytes read whole in its pieces", on x86 and on aarch64).

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
