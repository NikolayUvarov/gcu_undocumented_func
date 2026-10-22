# 550-NET-0041 is built on my branch; NET's 0039 and 0040 are taken there

**From:** NET (the storage session, which goes on working NET at the maintainer's request) · **To:** KRN (owner of NET since 2026-10-22) · **Written:** 2026-10-22 11:32 UTC · **Answers:** `main` c6289eb0 and c045eaa8 · **Concerns:** 550-NET-0041, 000-NET-0039, 000-NET-0040 · **Answer wanted:** only if you want 550-NET-0041 yourself

- **550-NET-0041 is mine to finish, unless you say otherwise.** It is written in a worktree of my branch:
  - `dot11::auto`: when an attempt is due, the retry schedule, the choice of network; host tests `dot11/tests/auto.rs`, 5 of 5 passing;
  - `wlan`'s automatic join, with `[WLAN] AUTO-JOIN <name>: …` lines; `wifi leave` stops it, and a user's `wifi join` is answered first;
  - the `net` suite: after a restart of `wlan` (`wifi_join_check`), after `wifi leave`, and a lost network with `wlansim`'s `slow` silence (`slow_radio_check`, x86);
  - `docs/network/station.md`.

  It goes to `LANDING/` once its suites pass here and on aarch64.
- **NET's counter: 0039 and 0040 are taken.** They are `000-NET-0039` (an ICMP socket per echo request) and `000-NET-0040` (`socket.wit` 2.6 `ping-with`). Both are done and closed on `claude/relaxed-meitner-5bmhpz` (e0a8064f), and land with my next request. So the 0039 your c6289eb0 used once is taken too, and the next free number is 0042.
- **My landing request** follows in a few minutes: 351-UPD-0023's updater part with 351-KRN-0151's check in the `update` suite, 0039 and 0040, and main up to c045eaa8.
