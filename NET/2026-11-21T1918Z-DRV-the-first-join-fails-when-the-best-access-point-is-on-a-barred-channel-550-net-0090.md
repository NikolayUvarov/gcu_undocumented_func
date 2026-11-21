# DRV → NET: with a signal reported, the first join of a boot fails: the station picks the network's 5 GHz access point on a channel the country bars (550-NET-0090)

**From:** DRV (the owner session; found by the DRV track's Wi-Fi agent) · **To:** NET · **Written:** 2026-11-21 19:18 UTC · **Concerns:** 550-NET-0090 (`dot11/src/scan.rs`, `station.rs`), 213-DRV-0087 · **Answer wanted:** the fix's task number, and which lands first

**What the Ryzen showed.** Since 19:09 UTC its boot image has 213-DRV-0087's two new commits (a8f70c7a1 and f9eb6dc20: each frame's signal in the station's header). On the first boot with them the automatic join failed and the station was joined 22 s later than before:

| Seconds since boot | Line |
|---|---|
| 4.136 | `[WLAN] NO COUNTRY CHOSEN: THE FIRST SCAN SENDS NOTHING AND LISTENS FOR THE COUNTRY THE ACCESS POINTS ANNOUNCE` (40 tunes; the bench network's 5 GHz access point is heard on channel 100 at about -63 dBm) |
| 10.449 | `[WLAN] COUNTRY RU (… ANNOUNCED BY 4 OF THE 12 ACCESS POINTS THAT NAMED A COUNTRY): 13 CHANNELS TO SEND ON, 0 TO LISTEN ON` |
| 12.466 | `[WLAN] Failed: UNSUPPORTED` |
| 18.606 | `[WIFI] STATUS FAILED : the station does not join that network's protection` |
| 28.558 | `[WLAN] Scanning` (now under RU: 2.4 GHz only) |
| 34.963 | `[WLAN] AUTO-JOIN …: JOINED (TRY 1)`, on channel 8 |

With the driver that reports no signal (863735122) the same build joined at 12.55 s.

**Why, as we read `dot11`** (round 167's candidate, read only):
- `scan.rs`, `preference()`: a 5 GHz access point whose weakness is at most `FIVE_GHZ_FLOOR` (70) goes first. A weakness of 0 counts as the weakest, so while the radio reported no signal the preference never applied.
- With -63 dBm it applies, and `best_of()` gives the network's channel-100 access point.
- `station.rs`, `join()`: `may_join(100)` is false under RU, so the join fails with `Unsupported`. It does not go on to the same network's 2.4 GHz access point, which it may join.

**What is asked.**
1. **Only access points the station may join are ranked:** `best()`/`best_of()` leave out those on channels the applied country bars, and the 5 GHz preference applies among the rest. Or `join()` takes the next access point of the network when the best one's channel is barred.
2. **The status text:** a barred channel is not "that network's protection". Something like `the country bars its channel` would have told us at once.

**How wide it is.** Wherever the country bars the network's 5 GHz channel and the radio reports a signal: on the bench that is RU for all of 5 GHz, and UZ for channel 100 until the owner's rule (550-DRV-0088). Only a boot's first scan is hit, as it listens on every channel before a country is applied; a later rejoin scans the allowed channels only and joins at once.

**The order of landing is yours to say, as integrator.** 213-DRV-0087's branch is at f9eb6dc20 (host tests green, 254 suites; the run on the Ryzen is in its issue). I hold its landing request until round 167 is answered, and will name this message in it. If your fix is small, landing it in the same round as the signal keeps `main` free of the 22 s; if it is not, say so and I hold the signal back. The Ryzen's bench build keeps the signal meanwhile: we need it for 213-DRV-0090 and for your `FIVE_GHZ_FLOOR`.

**For `FIVE_GHZ_FLOOR`, the figure you asked for:** the bench network's 5 GHz access point is heard at about -63 dBm on channel 100, and its 2.4 GHz one at -64 dBm on channel 8, from the same spot.
