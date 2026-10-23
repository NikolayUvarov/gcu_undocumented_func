# 550-DRV-0039 — `bcm_wifi` holds the BCM4331's external LNA off: the receiver no longer misses a fifth of strong frames

**Type:** driver · **Owner:** `DRV` · **Priority:** P1 · **Status:** in progress · **Blocked by:** — · **Main task:** [550](550-network-on-real-hardware.md), through [550-DRV-0006](550-DRV-0006-broadcom-wifi.md) · **Constitution:** MC-10.2, MC-12.1, MC-12.3

## Problem

[550-DRV-0036](550-DRV-0036-the-radios-traffic-in-the-log.md)'s counts on the MacBook Pro (2026-10-23) showed:
- **beacons missed in quiet periods:** the receiver missed 6% to 25% of the access point's beacons, mostly one by one, at a strong −44 to −46 dBm;
- **a sharp floor:** it heard no frame weaker than about −61 dBm;
- **episodes of interference:** about 80 s each, at 02:23 and 04:04 UTC. Frames went missing in runs while their level did not fall. They cost data frames even after 7 tries, and the Effector agent its connection.

The SPROM's board flags (0x1040_1201) name:
- an external 2.4 GHz LNA (`EXTLNA`);
- a front end shared with Bluetooth (`FEM_BT`);
- Bluetooth coexistence (`BTCOEXIST`).

`bcm_wifi`'s HT-PHY set-up follows b43's, and b43 neither sets those up for this board nor calibrates the HT-PHY's gain for an external LNA.

## What was found (the MacBook Pro, 2026-10-23)

The stick was `fast-test` 0d73c076, then EFF's release trees (slot B 31d66d3c, slot A d46435eb), all ABI 17. The test builds were put in with `replace` through `effector485`.

1. **The state as the radio came up:**
   - host flags 0000 0000 0000;
   - ChipCommon GPIO in 00000003, out 0, out-enable 0, control 0000000F;
   - the MAC's GPIO mask 000F;
   - the RF control overrides (PHY 0x84C, 0x86C, 0x88C) 0000 on every core.

   So the PHY drives the front end's lines itself.
2. **Chip control bit 2 (`EXT_LNA_G`) on:** no network heard at all.
   - The bit outlived the service's restart.
   - The replace after it left the 802.11 core with reset status 2 until a reboot. That was the stop path of 550-KRN-0157, asked of `KRN`.
3. **The RF control overrides swept,** each value for 3 s on every core, against the 3 s before it. A round of 14 values was run twice.
   - **0x0400** (the override on, every field 0), and any of bits 0–5, 8 or 9 with it:
     - nearly every beacon heard (26 to 28 in 3 s, of about 29 due), against 18 to 25 before;
     - the levels about 20 dB lower: frames read at −45 dBm before read −65 to −75.
   - **0x0440 (bit 6):** the receiver deaf, 0 beacons.
   - **0x0480 (bit 7):** as without the override: high levels, beacons missed.

   So bit 7 most likely powers the external LNA, and bit 6 most likely sets the transmit/receive switch to transmit.
4. **0x0400 held 60 s, in turn with the PHY's own control,** five times each. The first 10 s period after each change is left out:

| RF control | Beacons heard | Data frames lost after 7 tries | Tries a frame |
|---|---|---|---|
| 0x0400 held | 85 to 97 of 97 each 10 s, mostly 90 to 96 | about 1.7% | about 2.0 |
| the PHY's (0000) | 62 to 91 of 97, mostly 67 to 80 | about 4.8% | about 2.3 |

   Frames still go out with the override held.

5. **What it means.**
   - **The external LNA overloads the receiver.** Under the PHY's own control it is on, and it overloads the receiver on a strong access point.
   - **The levels read are not calibrated.** They include the LNA's gain when it is on, so they read about 20 dB high (MC-12.3).
   - **The "floor" was not a receiver limit.** Referred to the antenna it is about −81 dBm either way, which is likely the weakest network around rather than the receiver's sensitivity.

6. **Six values cycled, 60 s each, for 40 minutes** (05:20 to 06:00 UTC, slot A d46435eb). There were 28 periods of 10 s per value, the first after each change left out:

| RF control | Beacons heard | Frames received each 10 s | Data lost after 7 tries | Tries a frame |
|---|---|---|---|---|
| 0000, the PHY's | 83.3% | 525 | 15 of 822, 1.8% | 2.01 |
| 0400 | 95.6% | 604 | 15 of 683, 2.2% | 1.94 |
| 0410 | 95.6% | 611 | 10 of 693, 1.4% | 1.81 |
| 0420 | 96.4% | 617 | 8 of 683, 1.2% | 2.03 |
| 0430 | 95.4% | 617 | 11 of 683, 1.6% | 1.84 |
| 0480 | 84.9% | 526 | 7 of 657, 1.1% | 1.81 |

   - **Receiving.** Every value with the LNA off hears 95% to 96% of the beacons and 15% more frames, against 83% to 85% with it on.
   - **Sending.** No value differs beyond the counts' noise. 0x0400 is taken. Whether it lowers the transmitter's power is open: the access point's view of this station's level would tell.

## Plan

1. **Hold the override when the radio comes up** (done). `bcm_wifi` holds `RF_CONTROL_HELD` (0x0400) on every core. It logs it with the PHY's values from before, 2 s after it starts serving: logd refuses a service's records past 64 a second, which the radio's start passes. The levels then read without the LNA's gain.
2. **Whether a value keeps the transmitter's power** (cycled, step 6 above). b43 writes 0x0400 to turn the PA off for its idle TSSI measurement, so 0x0400 may also turn the power amplifier off. No value sent better than another. A measure of the transmitted level, at the access point, is still to be taken.
3. **The station is not affected.** `wlan` and `dot11` use the level only to rank networks against each other (`dot11/src/scan.rs`, `wlan/src/auto.rs`), so a shift of every level changes no choice. Only the dBm that `wifi status` shows drops. `NET` is told for the record.

## Acceptance criteria

- [ ] On the MacBook Pro, `bcm_wifi` logs the override held when the radio comes up. Over at least 20 minutes the network's beacons are heard at 90% or more in quiet periods, against 75% to 85% before.
- [ ] Data frames lost after 7 tries are no more than under the PHY's control, measured over the same period in turn.
- [ ] What the levels read now mean is written in this issue's resolution, and `NET` is told.

Evidence for the MacBook Pro's BCM4331 only (MC-12.1). Other boards with this chip may wire their front end otherwise.

## Related

[550-DRV-0036](550-DRV-0036-the-radios-traffic-in-the-log.md), [550-DRV-0029](550-DRV-0029-bcm4331-receives-ofdm.md), [550-DRV-0028](550-DRV-0028-bcm4331-transmit-power.md), 550-KRN-0157.
