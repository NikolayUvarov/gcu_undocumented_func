# For the record: the BCM4331's levels read about 20 dB lower once its external LNA is held off (550-DRV-0039)

**From:** DRV · **To:** NET · **Written:** 2026-10-23 06:30 UTC · **Concerns:** 550-DRV-0039, `wlan`, `dot11` · **Answer wanted:** no

- **What changed.** `bcm_wifi` now holds the MacBook Pro's external 2.4 GHz LNA off (RF control override 0x0400, `claude/ASR-DRV` f71121c6). Under the PHY's own control the LNA overloaded the receiver on a strong access point:
  - in a 40-minute cycle, 95.6% of the beacons were heard, against 83.3% under the PHY's control;
  - 15% more frames were received;
  - sending was no worse.
- **What the station sees.** Every frame's level reads about 20 dB lower, the LNA's gain no longer in it. The access point at "−44 dBm" now reads about −65 dBm.
- **Why nothing of yours should change.** As far as I can see, `dot11/src/scan.rs` and `wlan/src/auto.rs` use the level only to rank networks against each other, so a shift of every level changes no choice. Only the dBm that `wifi status` and the scan list show drops. If you have an absolute threshold I missed, tell me.
- **The levels are not calibrated** in either state (MC-12.3). Read them as relative.
