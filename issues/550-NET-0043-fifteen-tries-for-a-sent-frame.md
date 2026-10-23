# 550-NET-0043 — Fifteen tries for a frame the station sends

**Type:** `wlan` · **Owner:** network track · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Main task:** [550](550-network-on-real-hardware.md) · **Constitution:** MC-12.1

From DRV's request of 2026-10-23 (`NET/2026-10-23T0705Z-DRV-fifteen-tries-for-a-data-frame-under-bursts-of-interference.md` on `internal-dev-messages`). The maintainer's word, as DRV gives it: the MacBook Pro sits in interference that cannot be removed, and MIND must work through it.

## Problem

- **What `wlan` asks today.** At most 7 tries for each frame it sends (`ATTEMPTS`, [550-NET-0042](550-NET-0042-seven-tries-for-a-sent-frame.md)).
- **What DRV saw on the MacBook Pro** (BCM4331, its counting `bcm_wifi`, 550-DRV-0036 and 0039):
  - twice in one morning, a burst of about 80 s in which frames went missing in runs while the levels of those heard did not fall;
  - in the first burst, about one try in 4.5 got through (22 of 28 frames acknowledged after 100 tries);
  - at that rate 7 tries lose about 18% of the frames, and the counts showed about 20%;
  - the Effector agent lost its connection 8 times in that burst.

## Plan

- **The change.** `ATTEMPTS` becomes 15:
  - `bcm_wifi` and `rtw89_wifi` take 1 to 15 tries;
  - `wlanhw.wit` and `wlansim` stay as they are.
- **What it should give.** By DRV's figures, about 2.4% lost at the burst's rate, against 18%. This is an estimate from one burst's rate, not a measurement.
- **What it costs.**
  - In clear air, nothing: a frame took 1.8 to 2.0 tries on average in 550-DRV-0039's 40-minute cycle, so the limit is only reached when a frame would have been lost anyway.
  - In a burst, time: 15 tries take about 40 to 80 ms at 11 Mbit/s and up to about 200 ms at 1 Mbit/s. That is well inside the station's 2 s deadline for a radio call and under smoltcp's first resend at 1 s.

## Acceptance criteria

- [ ] The `net` suite passes on x86 and aarch64 with 15 tries (`wlansim` answers the tries it is asked for).
- [ ] On the MacBook Pro, the share of sent frames lost in a burst is lower than in the two bursts of 2026-10-23 with 7 tries. This is DRV's to record, and it is evidence for that machine, that access point and that interference only (MC-12.1).

## Related

[550-NET-0042](550-NET-0042-seven-tries-for-a-sent-frame.md), [550-NET-0018](../issues-done/550-NET-0018-the-station-service.done), 550-DRV-0036, 550-DRV-0039, `idl/wlanhw.wit` (`send`).
