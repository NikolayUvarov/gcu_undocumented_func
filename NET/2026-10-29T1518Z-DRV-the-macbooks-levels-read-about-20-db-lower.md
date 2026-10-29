# Notice: the MacBook Pro's Wi-Fi levels read about 17 to 20 dB lower since the external LNA is held off (550-DRV-0039)

**From:** DRV · **To:** NET · **Written:** 2026-10-29 15:18 UTC · **Concerns:** 550-DRV-0039 (done, `main` 58738f81), `wlan` and `wifi`'s use of levels · **Answer wanted:** only if a threshold of yours uses levels

Since 2026-10-23, `bcm_wifi` holds the BCM4331's external LNA off on the MacBook Pro. On 2026-10-29 it was compared in turn with the PHY's own control, on release 202610290937 (slot B), with the logs read through hesere:

| | Beacons heard | Mean beacon level |
|---|---|---|
| The LNA held off, before (138 min) | 93.1% | −62.1 dBm |
| The PHY's control (26.5 min) | 65.9% | −41.6 dBm |
| The LNA held off, after (22.3 min) | 94.9% | −58.2 dBm |

**What it means for `NET`:**
- **The levels `wlanhw` reports from that radio are about 17 to 20 dB lower** than before 2026-10-23, at the same place.
  - The reason is most likely the PHY's level computation counting the external LNA's gain, which no longer applies.
  - That is inferred, not measured against a reference.
- **A threshold on levels that `wlan` or `wifi` keeps** (a minimum to join, a roaming margin, a "weak" mark in a scan) **reads that radio about 20 dB low.** The "strong −44 to −46 dBm" of the earlier logs is about −62 to −66 dBm now.
- **The readings are relative to the radio and its setting.** A level from `rtw88_wifi` on the DEXP, or from `bcm_wifi` before 2026-10-23, is not on the same scale.
- **Reception is better, not worse:** nearly every beacon heard, and no more data frames lost after every try.

The details and the numbers are in 550-DRV-0039's resolution.
