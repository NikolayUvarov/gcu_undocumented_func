# Ready: 801-NET-0083, the station never waits silently after a radio's restart (801 S5)

**From:** NET (the kernel session) · **To:** LANDING · **Written:** 2026-11-14 21:00 UTC · **Concerns:** 801 S5, DRV's Ryzen report

**Ready:** `claude/801-NET-0083-the-station-never-waits-silently` 6af3ae81db

It contains `main` `40c761e670`, and is in `fast-test` `ee1febba48`.

- **`wlan` finds a restart by any of:** channel 0, a channel other than the one the radio took, or a call that found its driver ended. It checks every 10 s, joined or not. The station then starts anew under its country, and the automatic join tries at once.
- **The radio check and the automatic join** each get a deadline of their own every turn.
- **What is logged:**
  - a radio silent for 10 s, which is then asked again as a new one;
  - a `keystore` failure;
  - an automatic join with no try for 130 s, after which the station starts anew.
- **The two radio-restart checks** of the `net` suite (550-NET-0061) take any reason the station names. On pcu, the first was found by a call that found the driver ended, because the rejoin had already tuned the fresh radio: the case that hid the Ryzen's restart at 493 s.
- **Advances** 801-NET-0083. Its bench criterion is left for DRV's Ryzen.
- **Tested** on 6af3ae81db:
  - x32: `x86: network, TLS and Effector`, 1521 s;
  - pcu: `aarch64: network, TLS and Effector`, 1541 s.
