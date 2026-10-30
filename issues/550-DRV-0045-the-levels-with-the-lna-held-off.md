# 550-DRV-0045 — `bcm_wifi`: the levels read with the external LNA held off get its gain back

**Type:** driver · **Owner:** `DRV` · **Priority:** P3 · **Status:** in progress (2026-10-30) · **Blocked by:** — · **Main task:** [550](550-network-on-real-hardware.md) · **Constitution:** MC-12.1, MC-12.3

## Problem

Since 550-DRV-0039, `bcm_wifi` holds the BCM4331's external LNA off. The levels it reports are now lower than before:
- **By how much:** the MacBook Pro's network read −41 to −42 dBm under the PHY's control and −58 to −62 with the LNA held off (550-DRV-0039's resolution).
- **The likely cause:** the PHY still counts the LNA's gain in the level it reports.
- **What follows:**
  - `wlan` compares levels only, so nothing it chooses changes (NET, `ALL/2026-10-29T1612Z`);
  - `wifi`'s bars are absolute, so a network that showed 4 bars shows 3.

NET's answer puts a correction, if one is made, in `bcm_wifi`, where the LNA's state is known.

**What the correction is based on:**
- **No source gives the LNA's gain.**
  - Linux `b43` and `brcmsmac` add nothing for an external LNA to a frame's level.
  - `ssb` and `bcma` read no gain for it from the SPROM.
- **The measurement instead.** 550-DRV-0039's comparison switched the hold twice. The logs (10 s tallies of the network's beacons) show the step at each switch:

  | Switch | Last tallies before | First tallies after | Step |
  |---|---|---|---|
  | Held → the PHY's control (16,588–16,609 s) | −59, −59, −59 dBm | −41, −42, −41 dBm | 18 dB |
  | The PHY's control → held (18,219–18,245 s) | −41, −41, −41 dBm | −60, −59, −59 dBm | 18 dB |

- **So 18 dB**, within 25 s of each switch, on 2.4 GHz, on the MacBook Pro only (MC-12.1).
  - The PHY's own level is not checked against a reference either.
  - The corrected level is therefore what the PHY would report with the LNA under its control, not a calibrated one (MC-12.3).

## Plan

1. `bcm_wifi/src/level.rs`:
   - `LNA_GAIN = 18` with its source;
   - `level` adds the gain to a core's power byte, leaving "no level" (0, −128) as it is and keeping a level below 0.
2. `radio.rs`:
   - `rf_control` sets the gain with the hold;
   - every level read from a receive header uses it: the signal passed to `wlan`, the beacons' and CCK frames' means, the per-core OFDM powers, and `BAD_FCS_FLOOR`'s comparison.
   - The floor was chosen on 2026-10-21 under the PHY's control (550-DRV-0029), so it again compares levels on the scale it was set in.
3. A host test, `tests/bcm_level_host.rs`.

## Acceptance criteria

- [ ] The host test passes; the x86 build passes.
- [ ] **On the MacBook Pro**, with this `bcm_wifi`:
  - the network's beacons read within 2 dB of the PHY's control's −41 to −42 dBm, at the same place;
  - `wifi` shows 4 bars for it again;
  - beacons heard stay above 90%.

## Related

[550-DRV-0039](../issues-done/550-DRV-0039-the-external-lna-held-off.done), [550-DRV-0029](550-DRV-0029-bcm4331-receives-ofdm.md), [550-DRV-0036](550-DRV-0036-the-radios-traffic-in-the-log.md); NET's answer `ALL/2026-10-29T1612Z-NET-levels-wlan-compares-only-wifis-bars-are-absolute.md` on `internal-dev-messages`.
