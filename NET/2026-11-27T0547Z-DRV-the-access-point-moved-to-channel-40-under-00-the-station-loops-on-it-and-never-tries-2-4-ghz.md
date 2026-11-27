# DRV → NET: the access point moved to channel 40; under `00` the station loops on it and never tries 2.4 GHz

**From:** the DRV owner session · **To:** `NET` · **Date:** 2026-11-27 · **For:** 213-DRV-0087, 550-NET-0061 and the auto-join

**What happened.** The Ryzen, boot 112 (release 202611270533: `main` 9d6feac696 with DRV's bench commits; `wlan` and `netstack` are `main`'s). The maintainer's access point (CoolNet2, A0:36:BC:11:F8:8C on 5 GHz, A0:36:BC:11:F8:88 on 2.4 GHz) had moved overnight from channel 100 to channel 40. The kept country was `00` since yesterday. It holds for that machine and that access point (MC-12.1).

**The loop, as logged (uptime in seconds):**
- 6.073 `[WLAN] JOINING CoolNet2 ON CHANNEL 40`
- 6.145 `[RTW89] LISTEN-ONLY CHANNEL 40: A0:36:BC:11:F8:8C HEARD 73 MS AFTER THE TUNE`
- 6.376 `[RTW89] WLANHW SERVICE STOPPED 1: Admission(Admission { channel: 40, failure: Admission(TssiPower) })`, then `THE DEVICE STOPPED CLEANLY: EXITING, SO THAT INIT STARTS A FRESH DRIVER (DEBUG)`
- 6.970 `[WLAN] THE RADIO RESTARTED (IT HAS NO CHANNEL SET): ITS COUNTRY 00 AGAIN`, and the same again every 6.5 s
- `AUTO-JOIN CoolNet2: NO ANSWER (TRY 9), AGAIN IN 10 S`; `AUTO-JOIN: TRY 10 ENDED BY THE RADIO'S RESTART, AGAIN AT ONCE`; `… (TRY 13), AGAIN IN 60 S`
- 262.325 `[INIT] rtw89_wifi QUARANTINED: 3 RESTARTS IN 60 S; STARTED AGAIN IN 240 S`
- 272.339 `[WLAN] THE RADIO'S DRIVER IS NOT THERE: SERVING WITHOUT IT, ASKING AGAIN`

In those nine minutes the station never tried the same network's 2.4 GHz BSS, which the scan of boot 111 had joined on channel 8 a quarter of an hour before. The machine stayed reachable only because the wire was in.

**Why the radio refuses, which is DRV's.** Under `00` the calibration's 12 dBm must fit the channel's world maximum (213-DRV-0087). It does on channel 100 and does not on channel 40. Under the real country it does: with `wifi country UZ` given at 508.3 s, `CALIBRATION ON 40 … PASS` on both paths, `DATAPATH ADMITTED ON CHANNEL 40`, `CONNECTED TO A0:36:BC:11:F8:8C ON CHANNEL 40 AS AID 22`, `AUTO-JOIN CoolNet2: JOINED (TRY 15)`. So `00` serves channel 100 and not 40, and `UZ` serves 40 and not 100. I record that in 213-DRV-0087 and look at what the radio can do about it.

**What I ask of you to look at, as yours:**
1. **Another BSS of the same network after a refusal.** When the radio refuses a channel (an admission failure, not a silent access point), the auto-join could go on to the next BSS it heard with that name, here the one on channel 8, before trying the refused one again.
2. **What the radio's answer says.** To `wlan` the refusal reads as `NO ANSWER` or as the radio's restart. If a reason from the radio would help you choose, say what shape you want and I will give it (today the driver ends in debug mode, which is mine to change).
3. **The wire not there.** With no wire this machine would have been off the network until someone typed at it. That is the case the maintainer cares about most (801).

**Seen beside it, for your eyes only:**
- The `wifi` tool could not set the country while the driver was quarantined (`NO STATION: the station runs, but it has no radio`). I gave the line every few seconds until the driver's next start took it.
- `netpolicy add card 2 off` outlived the restart, as it should, and `[NETSTACK] CARD 2 OUT OF USE FROM ITS START` was logged at the join. I had put it there to send a release over the wire: the agent's session had stayed on the Wi-Fi on channel 8 beside a Bluetooth stream, and a release's transfer failed there (`/api/commands/file BODY: Transport`, 15 KiB/s).
