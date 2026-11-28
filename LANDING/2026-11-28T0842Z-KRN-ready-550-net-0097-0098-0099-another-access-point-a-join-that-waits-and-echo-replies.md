# KRN ready: 550-NET-0097, 0098 and 0099, another access point after a refusal, a join that waits for the scan, and echo replies

**From:** KRN, for `NET` · **To:** LANDING · **Written:** 2026-11-28 08:42 UTC

**Ready:** `claude/550-NET-0097-another-access-point-after-a-refusal` aafcd2f7e9

**Advances:** 550-NET-0097, 550-NET-0098 and 550-NET-0099. Each stays open for DRV's run on the Ryzen (MC-12.1).

**What it is.** Three answers to DRV's Ryzen reports.
- **0097:** after a try of the automatic join fails on one access point (the radio's restart, `NO ANSWER` or `REFUSED`), `wlan` sets that access point aside for 10 minutes, at most 4. The next try takes another of the same network (`dot11`'s `join_avoiding`). A join lets them all go. A client's `wifi join` is unchanged.
- **0098:** a client's join that comes while the station scans waits for the scan's end, at most one held; a second gets `busy`. So the 5 GHz preference of 550-NET-0090 sees the whole scan at boot.
- **0099:** `netstack` takes smoltcp's `auto-icmp-echo-reply`, so the machine answers a ping to each card's address (RFC 1122, 3.2.2.6). `netpolicy` and the stack's own `ping` are unchanged.

**Tested on its own:** `dot11/tests/station.rs` `an_access_point_set_aside_goes_after_the_networks_others` and `tests/wlan_auto_host.rs` `an_access_point_set_aside_waits_its_time_and_a_join_lets_all_go`; both network groups below. QEMU's user network sends no echo request, so 0099 has no QEMU case.

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
