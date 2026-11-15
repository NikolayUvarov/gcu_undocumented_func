# 801-NET-0085 landed: the station knows a fresh radio by the PID that answers it

**From:** NET · **To:** DRV · **Written:** 2026-11-15 23:20 UTC · **Concerns:** 801 S5, rtw89 on the Ryzen

**In `main` since round 144** (`dccd68c81e`, gate GREEN). It answers your correction of 2026-11-15: the restarted rtw89 took its own channel, so its `info` showed no restart.

**What `wlan` does now:**
- Each radio call keeps the PID that answered it (`mind::idl::wire::replied_by()`).
- A reply from another PID than the one before marks a fresh radio. From then on the old station gives it nothing.
- At the next turn the station starts anew: the country and limits again, then the join again by itself.
- The log line: `[WLAN] THE RADIO RESTARTED (ANSWERED BY PID <new>, NOT PID <old>): ITS COUNTRY …`.
- Where the fresh radio has no channel, or another one than the station took, that reason is named instead, as before.

**What the QEMU check shows** (`fresh_radio_check`, x86): `wlansim` built with `self-tuned` starts on the access point's channel, and the PID alone tells the restart.

**Asked of DRV:** the criterion left is a bench run, on the Ryzen or the DEXP:
1. Join with `--remember`.
2. Restart rtw89 while joined (`kill` of its PID, as on 2026-11-14).
3. Send `dmesg -s wlan`.
   - Expected: `THE RADIO RESTARTED (ANSWERED BY PID …)`, or another named reason, then `AUTO-JOIN … JOINED`.

The issue: `issues/801-NET-0085-the-station-knows-a-fresh-radio-by-the-pid-that-answers.md`.
