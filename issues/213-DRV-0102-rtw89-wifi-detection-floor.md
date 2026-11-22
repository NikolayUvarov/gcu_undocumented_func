# 213-DRV-0102 — `rtw89_wifi`: the receiver's detection floor as Linux's DIG holds it

**Type:** driver (`rtw89_wifi`) · **Owner:** `DRV` · **Priority:** P1 · **Status:** open (2026-11-22) · **Blocked by:** — · **Main task:** [213](213-amd-ryzen-pc.md) · **Constitution:** MC-6.1, MC-12.1, MC-12.3

## Problem

On the Ryzen, sends are held in the MAC and then sent once and acknowledged (213-DRV-0091: boot 82 at 22% of sends, every other boot at a low rate).

**The holds follow the channel's load.** In the series of starts of 2026-11-22, boots 84 to 89, the late frames a minute rose with the PPDU reports heard a minute:

| PPDU reports a minute | Late frames a minute |
|---|---|
| 23.5k | 0 |
| 27.0k | 3 |
| 30.2k | 8 |
| 32.5k | 11 |

**Our detection floor is far below Linux's.** This driver never ported DIG, so the receiver keeps the floor the initial PHY set:
- the OFDM packet-detection lower bound at 0 (−102 dBm), spatial reuse off;
- the CCK lower bound at 0, and the CCK limit not enabled;
- EDCCA's PPDU level at 56.

Linux's DIG and EDCCA at the joined access point's −60 dBm hold:
- the OFDM bound at 9 (−84 dBm), with spatial reuse on;
- the CCK bound at −83 (0xAD), with its limit on;
- EDCCA 66 on all three fields.

So our receiver may count faint frames of distant networks as a busy medium where Linux would not. Linux does 480 to 826 KB/s on the same channel in the same room.

## What Linux does (pinned source at 05f7e89)

- **The RSSI.** `rtw89_phy_dig_update_rssi_info` (phy.c:6454) sets `igi_rssi = ch_info->rssi_min >> 1`, with RSSI raw (dBm + 110) × 2 (core.h:52, RTW89_RSSI_RAW_TO_DBM). At −60 dBm, `igi_rssi` is 50.
- **The FA offset.** `rtw89_phy_cal_igi_fa_rssi` (phy.c:6851) adds a false-alarm offset to `igi_fa_rssi` and clamps it to `igi_rssi` − 10 .. + 25. But `__rtw89_phy_dig_dyn_pd_th` (phy.c:6768, the min at 6781) takes `final_rssi = min(rssi, igi_rssi)`, so the PD bound never rises above what `igi_rssi` gives.
- **The OFDM bound.** Under region = `pd_low_th_offset` 16 (phy.c:6498) + PD_TH_SB_FLTR_CMP_VAL 7 + PD_TH_BW20_CMP_VAL 0 (phy.h:120, 122; `rtw89_phy_dig_cal_under_region`, phy.c:6737).
  - `ofdm_cca_th = clamp(final_rssi, PD_TH_MIN_RSSI 8 + 23, PD_TH_MAX_RSSI 70 + 23)` (phy.h:114, 115).
  - `pd_val = (ofdm_cca_th − 23 − 8) >> 1`, written into R_SEG0R_PD_V1 0x4860's B_SEG0R_PD_LOWER_BOUND_MSK [10:6], with the spatial-reuse enable BIT(30) (`rtw89_phy_dig_dyn_pd_th`, phy.c:6799; reg.h:8683 to 8692; rtw8852b.c:218 to 220).
- **The CCK bound.** `cck_cca_th = max(final_rssi − 23, CCKPD_TH_MIN_RSSI −18)` (phy.h:116), and `pd_val = cck_cca_th − IGI_RSSI_MAX 110` (phy.h:113).
  - Written into R_BMODE_PDTH_V1 0x4B64 [31:24], with B_BMODE_PDTH_LIMIT_EN_MSK_V1, BIT(30) of 0x4B74 (reg.h:8821 to 8826; rtw8852b.c:221 to 224).
- **EDCCA.** `rtw89_phy_edcca_get_thre_by_rssi` (phy.c:7860), when linked: `rssi_min >> 1` − RSSI_UNIT_CONVER 110 + EDCCA_UNIT_CONVER 128 − EDCCA_TH_REF 3, at least EDCCA_TH_L2H_LB 66 (phy.h:131 to 135).
  - `rtw89_phy_edcca_thre_calc` (phy.c:7879) writes it into R_SEG0R_EDCCA_LVL_V1 0x4884's [7:0], [15:8] and the PPDU level [31:24] (reg.h:8686 to 8689; rtw8852b.c:242 to 246).
- **Linux tracks all of this.** DIG and EDCCA run from the periodic track work and follow the RSSI and the false alarms.

## Plan

One change, one measurement:

1. **`service::detection_floor`** (this branch):
   - Linux's arithmetic above as functions of the joined access point's RSSI.
   - The switch `DETECTION_FLOOR_AS_LINUX`, committed false (the driver as it was). The bench sets it true.
   - With it true, the floor is written once the joined BSS's beacons are measured: the mean signal of 8 of them, from the PPDU status report after each. It is written on a 2.4 GHz 20 MHz channel only, and again after a recovery the data mode outlasted or a new link.
   - On any other band the floor is not written, and one line says so. No values are guessed for a band never joined.
   - **It is a static floor, not a DIG (MC-12.3).** It holds what Linux would set at the RSSI measured when the link came. It has no false-alarm offset and no periodic update. Those are the open part, and come only if the static floor shows an effect.
2. **The minute lines say what the four registers read.** A floor the firmware or a recovery takes back shows itself. They also give every PPDU report by its signal in 5 dB bins from −20 dBm down, with the last bin below −100. The probes read 0x4B74 as well.
3. **The measurement.** The series of starts with the switch false (2026-11-22, boots 90 on) against the same series with it true, compared by:
   - late frames per 10k PPDU;
   - late frames per send (the PPDU count itself should fall if fewer frames are detected);
   - hold lengths;
   - the histogram's faint share.

## Acceptance criteria

- [x] The arithmetic as Linux's, with host cases at −60 dBm (9, 0xAD, 66) and at each clamp's edge.
- [x] Written once measured, read back, and counted in the minute line, in a host case with the switch on.
- [ ] The two series compared on the Ryzen (MC-12.1: that machine, that channel, that hour), each figure above.
- [ ] If it helps, the open part (the false-alarm offset, the periodic update) decided on its own evidence.

## Progress

**2026-11-22, written** (branch `claude/213-DRV-0102-rtw89-wifi-detection-floor`, on 213-DRV-0091's b123adabf; host cases, not run):
- **`rtw89_wifi/src/detection_floor.rs`** (a submodule of `service`): `igi_rssi`, `ofdm_pd`, `cck_pd`, `edcca`, `for_rssi`, and the fields written (`fields`).
- **`data_mode`:**
  - The dispatcher remembers the joined BSS's beacon by its frame number, count and rate. When the report after it has the same count and rate, its signal measures the BSS (`Counts::bss_dbm`).
  - Every report goes into the 5 dB histogram, and an unparsable one is counted.
  - New: `floor_registers()`, `write_floor()` (each field read back) and `take_ppdu_rssi()`.
  - PROBE_BB and HELD_BB read 0x4B74, and HELD_BB also 0x4860 and 0x4B64.
- **`service`:**
  - The floor is due at a link, and again after a recovery once it was set. It is written when due, with no send pending and 8 beacons measured.
  - New lines:

    ```
    [RTW89] DETECTION FLOOR ON c AS LINUX FOR THE AP AT d DBM (n BEACONS): OFDM PD p (-x DBM), CCK PD v (y DBM), EDCCA e (z DBM); 4860=… 4B64=… 4B74=… 4884=… (213-DRV-0102)
    [RTW89] DETECTION FLOOR NOT SET ON CHANNEL c: ITS VALUES ARE KNOWN FOR 2.4 GHZ 20 MHZ ONLY (213-DRV-0102)
    [RTW89] TRAFFIC ON c: DETECTION FLOOR 4860=… 4B64=… 4B74=… 4884=…; THE AP AT d DBM OVER n BEACONS; SET AS LINUX: YES|NO (213-DRV-0102)
    [RTW89] TRAFFIC ON c: PPDU BY SIGNAL, 5 DB BINS FROM -20 DBM: [...] (213-DRV-0102)
    [RTW89] TRAFFIC ON c: PPDU BY SIGNAL, ON FROM -65 DBM: [...], THE LAST BELOW -100; UNPARSED u (213-DRV-0102)
    ```

- **Host cases** (tests/rtw89_service_cases.rs):
  - `the_detection_floor_is_linuxs_arithmetic_for_the_aps_rssi`;
  - `service_writes_the_detection_floor_once_the_aps_beacons_are_measured`. The switch is on through a test hook; eight beacons with their captured −63 dBm report give OFDM 8, CCK 0xAA and EDCCA 66, read back and in the minute line; the histogram's −60 to −64 bin holds them.
  - The 40 `rtw89_*_host.rs` files pass, and the release build builds.
- **Not covered by a host case:** the 5 GHz refusal, since the cases run on channel 6. It is one line, read from the code.

## Related

[213-DRV-0091](213-DRV-0091-the-firmware-told-of-the-link.md) (the held frames, the starts series), [213-DRV-0090](213-DRV-0090-the-receiver-goes-deaf-on-a-busy-channel.md) (the busy channel), [213-DRV-0032](213-DRV-0032-rtl8852be-wifi.md) (the driver; DIG, EDCCA and AIFS left).
