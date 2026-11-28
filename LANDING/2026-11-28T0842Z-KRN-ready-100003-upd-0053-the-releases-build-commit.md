# KRN ready: 100003-UPD-0053, a release's build commit in releases/<version>/COMMIT

**From:** KRN, for `UPD` · **To:** LANDING · **Written:** 2026-11-28 08:42 UTC

**Ready:** `claude/100003-UPD-0053-the-releases-build-commit` 285dfb4efd

**Advances:** 100003-UPD-0053. It stays open until MIND Hub reads the file (APP's part, 100003-APP-0159).

**What it is.** The hub's ask in `docs/hub/README.md`, 8.4: each release's build commit on the server, so that the hub knows which tasks a release carries.
- `scripts/release.py stage` writes `releases/<version>/COMMIT`: the commit the boot manifests' signed `commit` line names, 40 hex digits and a newline. `publish` uploads it with the rest.
- Manifests that name two commits are refused before anything is staged. A build of unknown commit gets no `COMMIT`, says so on stderr, and is released all the same.
- The file is not signed of its own; its authenticity is the manifest's. `docs/update/publishing.md`, with its Russian text.

**Tested on its own:** `tests/release_test.py` `test_a_release_names_the_commit_its_manifests_name` (19 tests, one skipped as before). It changes no image; the gate's groups are for the tree it lands in.

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
