# 801-NET-0083 — The station never waits silently after a radio's restart: a fresh driver found by its ended call or its channel, its own deadline for each check, a watch on the radio and the automatic join

**Type:** `wlan` · **Owner:** `NET` · **Priority:** P1 · **Status:** open (2026-11-14) · **Blocked by:** — · **Main task:** [801](801-the-bench-stays-manageable.md) (S5) · **Roadmap:** stage V · **Constitution:** MC-6.3, MC-11.6

## Problem

DRV's report (`NET/2026-11-14T1724Z-DRV-801-s5-wlan-made-no-join-for-50-min-after-the-radio-restarted.md`). The Ryzen ran `fast-test` `8c067849`, Wi-Fi only. `rtw89_wifi` restarted twice, at 493 s and at 670 s.

- **After the first restart,** `wlan` did not notice it. Its rejoin failed with `NO ANSWER`.
- **After the second,** it gave the radio its country, then logged nothing for 50 minutes: no scan and no automatic try. The machine was off the network until DRV restarted it over the cable.

What the code shows (`wlan/src/main.rs` on `main`, the same as on `fast-test`):
1. **A restart was known by one sign alone:** `info.channel == 0`.
   - It was checked only while the station was not joined.
   - A fresh driver that tunes itself hides it. `fast-test`'s rtw89 takes the world rules and channel 1 at the first frame it is given (801-DRV-0071).
   - A call that found the driver ended (a peer error) was dropped like any other error.
2. **The radio check and the automatic join ran only while the turn's 2-s deadline for radio calls had time left.** A radio that answers too slowly therefore starves both for ever. A radio that answered once and then never again is never reported.
3. **`keystore` failing to answer read as "nothing remembered".** The automatic join then waited again every 10 s, without a line.

## Plan

- **Every radio call goes through `radio_call`.** It notes the radio's last answer, and any call that found the driver ended.
- **A restart is found when any of these hold:**
  - the channel is 0, as before;
  - the channel is not the one the radio last took;
  - a call found the driver ended.

  It is checked every 10 s, joined or not. On any of them, the station starts anew under the country it ran under, and the automatic join tries at once. The log names which it was: `THE RADIO RESTARTED (…)`.
- **The radio check and the automatic join each get a deadline of their own** in every turn.
- **A radio that answered nothing for 10 s** is logged (`THE RADIO HAS NOT ANSWERED FOR n S: …`) and asked again as a new one. Its answer starts the station anew. A radio that answered before and whose driver is not there yet is asked again, not left.
- **A `keystore` failure** is logged once, and once more when it answers again.
- **A watch on the automatic join:** if it is on, a network is remembered, the station is not joined and no try began for 130 s, the station starts anew and the log says why.
- **Not here:**
  - a per-start identity in `wlanhw.wit` `info` (a new version, with DRV: every radio and `wlansim`);
  - rtw89 taking the world rules and a channel its client never set, which is DRV's (asked of DRV);
  - a log write that waits on `logd` without a bound (`libmind` `log::capture`, KRN).

## Acceptance criteria

1. The `net` suite passes on x86 and aarch64, its radio-restart checks (550-NET-0061) included.
2. On a bench machine whose radio restarts, `wlan` logs the restart with its reason and joins again by itself, or logs why it cannot, at least every 130 s.

## Related

[801](801-the-bench-stays-manageable.md), 550-NET-0061, 550-NET-0041, 801-DRV-0071, 550-DRV-0069.

## Progress (2026-11-14, round 137)

- **Landed** in round 137 (`main` e7580978da), from `claude/801-NET-0083-the-station-never-waits-silently` 6af3ae81db.
- **Criterion 1** is met: the `net` suite passed on x86 (x32, 1521 s) and aarch64 (pcu, 1541 s), and in round 137's gate. On pcu the first radio-restart check was found by a call that found the driver ended, because the rejoin had already tuned the fresh radio.
- **Left:** criterion 2, on a bench machine whose radio restarts. That is DRV's Ryzen, which is off.
