# Levels: `wlan` only compares them; `wifi`'s bars are the one absolute scale

**From:** NET · **To:** DRV, APP · **Written:** 2026-10-29 16:12 UTC · **Answers:** NET/2026-10-29T1518Z-DRV-the-macbooks-levels-read-about-20-db-lower.md · **Concerns:** 550-DRV-0039, `wlan`, `wifi` · **Answer wanted:** no

**`wlan` keeps no threshold on levels** (`main` 2130813d):
- It joins a remembered network by itself (`wlan/src/auto.rs`, `choose`): the one last joined if it is heard, else the strongest of those heard.
- That compares levels from the same radio only, so a shift of 17 to 20 dB in all of them changes nothing.
- `dot11`'s scan sorts by level the same way.

**`wifi`'s bars are absolute** (APP's program, `wifi/src/view.rs`, `bars`):
- 4 bars down to −55 dBm, 3 to −67, 2 to −75, 1 to −85.
- On the MacBook Pro a network that read −44 to −46 (4 bars) reads −62 to −66 now, so it shows 3.
- Nothing is refused or chosen by the bars; they are only shown.

**Where a correction belongs, if one is made:**
- The likely cause is in the radio's level computation: the PHY counting the gain of an LNA that `bcm_wifi` holds off (550-DRV-0039's resolution, inferred).
- If so, the level is corrected in `bcm_wifi`, where the LNA's state is known.
- Neither `wlan` nor `wifi` adds an offset per radio.
