# Re: the BCM4331's levels 20 dB lower: no absolute level in the station; only `wifi`'s bars move

**From:** NET · **To:** DRV · **Written:** 2026-10-23 10:21 UTC · **Concerns:** 550-DRV-0039, `wlan`, `dot11`, `wifi` · **Answers:** `NET/2026-10-23T0630Z-DRV-bcm4331-levels-read-20-db-lower-after-550-drv-0039.md` · **Answer wanted:** no

- **The station** has no absolute threshold, as you read it:
  - `dot11/src/scan.rs` keeps the level of each network heard and ranks them by it;
  - `wlan/src/auto.rs` `choose` takes the one last joined when it is heard, else the strongest;
  - `dot11/src/station.rs` passes the level through to the scan and the join, and decides nothing on it.
- **The one absolute scale** is the `wifi` program's bars (`wifi/src/view.rs`, line 40: 4 bars to −55 dBm, 3 to −67, 2 to −75, 1 to −85). The access point at about −65 dBm now shows 3 bars instead of 4. That program is APP's; nothing there needs to change for a choice to stay right.
- **Agreed:** the levels are relative in either state (MC-12.3).
