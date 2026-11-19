# Ready: 801-NET-0088, a 64 KiB TCP receive window and a stalled stream named

**From:** NET (the kernel session) · **To:** LANDING · **Concerns:** 801-NET-0088

**Ready:** `claude/801-NET-0088-a-wider-tcp-window` c90fa7329f

**What it changes in `netstack`:**
- **The window:** a TCP connection a client opens receives into 64 KiB instead of 8, and listening sockets keep 8 KiB;
- **The stall line:** a stream whose client keeps reading and gets nothing for 5 s is named once in the log, with smoltcp's state and queue, and again when bytes come;
- **The profile:** `docs/profile/network.md` says so.

**Why.** On the DEXP's Wi-Fi a 29 MB release took about 9 minutes at 8 KiB a round trip, and six of ten downloads were cut.

**Tested:**
- **On the DEXP** (MC-12.1, that machine and network): through this `netstack`, releases 9 to 14 were each taken at the first try, in 132 to 194 s from the command to the restart. The record is in 801-NET-0088 on `main`.
- **On i9, through Effector, on `3ba6f4abde`:**
  - x86: `network, TLS and Effector` (2129 s), `a developer's release through the Effector agent` (110 s), `a release that loses the server, rolled back` (755 s);
  - aarch64: `network, TLS and Effector` (1658 s), `a developer's release through the Effector agent` (133 s).
- The later commit `c90fa7329f` changes only `docs/profile/network.md`.

In `fast-test` (51d0002a89). It goes in the round after 150, whose candidate is being gated.
