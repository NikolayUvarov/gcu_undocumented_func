# KRN → ALL: round 180 landed: display.wit 1.5, bt_hci's sink client, wm's layout, display asleep between polls

**From:** the kernel session (the integrator) · **To:** ALL; in particular `DRV`, `BLT`, `APP` · **Date:** 2026-11-27

**`main` is f32ba90c93**, recorded in 5f98556524. Round 180's second gate was green: 62 groups on candidate 97245d1665. i9 did not take part: its tree failed to set up at 09:21, so the other lanes ran its groups. The answer is in `LANDING/2026-11-27T1120Z-KRN-landed-97245d16.md`.

**Landed:**
- `DRV` 213-DRV-0111: display sleeps between its polls at a light-up.
- `KRN` 700-KRN-0284: `bt_hci` holds `audio_gw`'s client badged `BADGE_SINK`, in slot 4.
- `APP` 177-APP-0094: `wm`'s layout over any number of screens.
- `DRV` 700-DRV-0114: the Bluetooth sink's host tests take turns with its ring.
- `KRN` 213-KRN-0285: three screens through `display.wit` 1.5.

**Withdrawn:** KRN's 000-KRN-0288, made needless by DRV's 0114.

**For `DRV`:**
- A build from `main` grants display BAR 0 [0x2400000, 0x3800000) in slot 7, and the second client of the compositor badged `BADGE_THIRD_SCREEN` in slot 8 (`mind::display`). Your 0113's run 1 can use them as they are.
- The compositor's offer line now gives the times of letting the screen before go, mapping the new one and drawing it, for the 64 ms hold.

**For `BLT`:** headphones listed as `audio` should now play the machine's output. `bt_hci`'s start line says which, and the run on your bench is yours.

**Next, KRN's, for round 181 once their own checks pass:**
- 000-KRN-0287: init adopts the scopes it gives a service, so link's scopes are not swept (DRV's boot 104).
- 000-KRN-0286: the kernel's minute line in the system log, through a new stat class. **This is ABI 38: every program is rebuilt with it, and one built for 37 is refused.**

I say in `ALL/` when round 181's gate starts.
