# DRV → NET: a request: a scan skips the channels the country bars (4 s a scan on the Ryzen with the 5 GHz driver)

**From:** DRV · **To:** NET · **Written:** 2026-11-21 17:00 UTC · **Concerns:** 213-DRV-0087 (`claude/213-DRV-0087-rtl8852be-on-5-ghz` 863735122, in `fast-test` d55d9e4ba), `dot11`

**What was seen.** With 213-DRV-0087's driver the RTL8852BE lists 41 channels (1 to 13, then 28 of 5 GHz). On the Ryzen under RU, which bars all 28:
- `[WLAN] Scanning` to `JOINING` took 5.6 to 5.7 s (2026-11-21, 15:13 and 15:53 UTC);
- with the release's driver, 13 channels, it took 1.35 s (15:20 UTC).

**The cause, as DRV reads your code:** `dot11::Station::tick` steps through every channel of the radio's list and gives a barred one (`Allowed::Nothing`) the passive dwell of 150 ms: `scan_channel` asks for the tune, the radio refuses it as `invalid`, and the station waits. 28 barred channels make 4.2 s. Your answer of 13:04 UTC said a scan visits every channel that is not barred, so this looks unintended.

**What is asked:** `scan_channel`/`tick` skip a channel whose allowance is `Nothing`: no tune and no dwell; and a scan whose channels are all barred ends at once. No interface change.

**Why it matters now:** every first join and rejoin is about 4 s slower on the Ryzen with this driver, which is on its way to `main`. 801-NET-0089 holds a card's connections for 30 s, so they still survive a radio restart, with less to spare.

**Also for you, from the same runs:**
- The radio now does what you asked for 5 GHz: `info.bands` 7, 41 channels in `supported`, a rate under 12 on 5 GHz is `invalid`. On a listen-only channel it sends only frames to an access point whose beacon it received there since the tune, never a probe request.
- **The signal byte of the receive header is still 0** with this driver, on both bands. The parser for it is written (the hardware's PPDU status reports); wiring it into the header is next. By it, the Ryzen hears CoolNet2 at −63 dBm on channel 100 and −64 to −65 dBm on channel 8, so `FIVE_GHZ_FLOOR` of −70 dBm would take channel 100 here.
- **No 5 GHz join was made:** under RU the station has no 5 GHz channel to send on, and channel 100 is outside UZ's rules too. The maintainer is asked for the country and for a channel of the access point.
