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

**2026-11-22, the floor alternating by the minute** (the same branch; host cases, not run):
- **Why.** The late frames follow the hour (213-DRV-0091). So a floor-off series against a floor-on series an hour later would compare two hours, not two floors.
- **`detection_floor::DETECTION_FLOOR_ALTERNATES`**, committed false.
  - With it and DETECTION_FLOOR_AS_LINUX true, the driver reads the four registers once the joined BSS's beacons are measured, before any write: the values found.
  - It announces the alternation:

    ```
    [RTW89] DETECTION FLOOR ON c TO ALTERNATE BY THE MINUTE, THE NEXT ONE OFF: LINUX'S OFDM PD …, CCK PD …, EDCCA … FOR THE AP AT d DBM (n BEACONS); FOUND 4860=… 4B64=… 4B74=… 4884=… (213-DRV-0102)
    ```

  - From the first whole minute after that, it holds the values found for a minute, then Linux's for the next, turn and turn about.
  - It switches at the minute line's boundary, at the first poll with no send pending.
  - Each field is read back at every switch. Linux's own DIG rewrites these fields every two seconds while it runs.
  - A link or an outlasted recovery writes the minute's kind again.
- **Each minute's floor line names what that minute held,** `THIS MINUTE FLOOR ON` or `THIS MINUTE FLOOR OFF`. Its registers are read at the minute's end, and a switch not as written is said.
  - The PPDU histogram runs in both kinds of minute. The OFF minutes give the faint share the old baseline lacks, and the ON minutes show what stops being detected.
- **The measurement:** one series of 5-minute starts with both switches true, reported as paired minutes. Each ON minute is set against the mean of its OFF neighbours, by:
  - late frames per send;
  - the late frames' median hold;
  - PPDU;
  - the faint share.

  Adjacent minutes of one boot cancel the hour and the start.
- **Host case** `service_alternates_the_detection_floor_by_the_minute`, on the model's clock:
  - the partial minute, then off, on, off;
  - the values found unchanged while off, and restored exactly after on;
  - Linux's fields while on;
  - a send pending across a boundary keeps the floor until the next poll after its release, not until the next minute.

  The 40 `rtw89_*_host.rs` files pass, and the release build builds.

## For a successor (state on 2026-11-22, 14:40 UTC)

Everything before this afternoon is in [213-DRV-0091](213-DRV-0091-the-firmware-told-of-the-link.md)'s section "For a successor (state on 2026-11-22)": the branches, the bench switches, the collector, the traps, and the answer path's six steps with their files and lines. What follows is what the afternoon added.

`S` below is the session's scratchpad: /tmp/claude-1000/-mnt-c-common-virtual-Culture-gcu-undocumented-func/4c527436-e074-4262-82a9-b8ea9d807c88/scratchpad.

**Heads:**
- `claude/213-DRV-0091-the-recoveries-placed` at b123adabf:
  - the SER steps placed;
  - the ring's polls and gaps;
  - the held-frame probe and the minute's sends;
  - the calibrations judged by Linux's checks;
  - `PORT_AS_LINUX`;
  - the held registers in pieces.
- This branch, `claude/213-DRV-0102-rtw89-wifi-detection-floor`, at 65870f63f plus this text: the floor and its alternation. In the bench tree as 82e458e2f with both switches true: release 202611221429, boot 93.
- Full suites:
  - 2e854da9c: 261 passed, 0 failed.
  - 1bcb225bc: a run in progress at this writing, /tmp/claude-1000/wf/full-a-floor.log. The tree changed under it, so it does not count for 65870f63f.
  - 65870f63f: queued after it, /tmp/claude-1000/wf/full-a-floor2.log.

**The held-frame probe (213-DRV-0091) and its lines:**
- `HELD FRAME (LATE n): …` gives the time since submission, and where in the beacon interval the frame was submitted and is now, by our clock and by the TSF.
- `HELD FRAME'S REGISTERS k: …` and, once, `IN-TIME FRAME'S REGISTERS k: …`, in 150-byte pieces (b123adabf; before it the lines were cut at 200 bytes, after D220). Join the pieces before reading.
- Each minute:
  - `LATE FRAMES RELEASED …` (first try, never, median hold, frame buffer full spells);
  - `RELEASES BY THE BEACON INTERVAL'S PHASE …`;
  - `RX POLLS …` and `RX POLL GAPS …`;
  - with this branch, `DETECTION FLOOR …` (the four registers, the AP's dBm, THIS MINUTE FLOOR ON or OFF) and two lines of `PPDU BY SIGNAL …`.

**The logs:**
- Boot 82, read from the stick: S/handoff/wifi/boot0082-from-the-stick.log.
- Boots 74 to 83, by the collector: S/handoff/wifi/day/.
- The first starts series (130 s each, floor as found, the cut probe), boots 84 to 89: S/handoff/wifi/starts/.
- The second series (5 minutes each, b123adabf, the whole probe): boot 91 in S/handoff/wifi/starts/. Boot 90 was left to run 5 minutes; its log was not there at this writing.
- Boot 93 (one long boot until 15:40 UTC, the floor alternating): read through the server as `log:/boot0093.log`. The owner's read at 126 s is S/own-draft/boot93-a.log.

**The scripts,** kept where a restart of this PC does not wipe them: /home/un/.local/share/mind-bench/wifi/. Copies are in S/wf-bench/.
- **`collect-day.sh FIRST_BOOT READS`:**
  - Every 30 minutes it fetches the current boot through the server into S/handoff/wifi/day/, skipping while anyone but WIFI holds the machine.
  - It exits at the first new CHECK MET, SER_DBG_INFO STEP, HISR0, error code, recovery, stop or hole line; `seen.txt` holds those already reported.
  - Start it in the background (`bash collect-day.sh 93 40`); stop it by stopping that task.
- **`starts_table.py LOGS…`**, one start a row:
  - the build and the port;
  - minutes, late of sent in the first 5 minutes and in all;
  - the late frames' outcomes, the frame buffer, RXQ's backlog;
  - the calibrations with DPK and LOK values;
  - each held probe's hold and phases.
- **`held_compare.py LOGS…`:** each minute's PPDU, late and late per 10k PPDU, and the held probes' registers against the in-time one (from the joined pieces), the differing ones only.
- **Other scripts:**
  - `day_report.py`: a boot's drivers, SER_DBG_INFO steps, meetings, minutes and new line kinds;
  - `windows.py`: ten-minute windows, SER rises against the minute lines;
  - `traffic_minutes.py`: beacons and late a minute;
  - `ser_per_driver.py`;
  - `probe_values.py KIND LIMIT LOGS…`: a probe's pieces joined, the values that differ between logs;
  - `watch-check.sh`: a cable watcher, lease-aware;
  - `hosttests.sh`: the rtw89 host files, its path and output changed with sed for each worktree;
  - `log.sh`, `ef.sh`, `get.sh` and `putsrv.py`: the cable bench, under a lease only.
- A one-off decode of boot 91's EDCCA reports (0x173C: FB bit 7, P20 bit 6, full-band power in byte 3, as Linux's `rtw89_phy_edcca_log`) was run inline. It is described below.

**The finding as it stands:**
- **Held, not lost.** Late frames are held in the MAC after the TX DMA fetched them, then sent once and acknowledged. Every boot has them; boot 82 had 22% of its sends.
- **The hour, not the start.** Over boots 84 to 89, late frames a minute rose with the PPDU heard a minute: 23.5k gave 0, and 32.5k gave 11.
  - But boot 93's first minute had 13 late at 27.7k. The PPDU count is not the whole of "busy".
- **No register of the MAC's TX path, the port, the TBTT prohibit, MACID pause or sleep, CTN_TXEN, TX collision or the coexistence set differs between a held frame and one in time** (boots 84 to 91).
- **Boot 91's EDCCA snapshot:** at 7 of its 13 holds the channel was busy by energy, −53 to −62 dBm with FB and P20 asserted, at or above the EDCCA level 66 (−62 dBm). That is as strong as our own access point (−61 dBm).
  - At the other 6 it read −68 to −81 dBm, and at the in-time frame −80.
  - Linux's EDCCA level at this RSSI is the same, and its −84 dBm PD floor still detects such frames. So those holds would be Linux's too.
- **The histogram** (boot 93, the owner's read):
  - The bins from −25 to −84 dBm hold the real distribution. Almost nothing falls between −85 and −100 (4 a minute).
  - So a floor at −84 dBm has almost nothing to cut in what the histogram measures.
  - The first bin (−20 or stronger, about 4800 a minute) and the last (below −100, about 500–580) are not real signals. They are reports with no RPL: OFDM PPDUs not addressed to this radio, whose IE 1 the parser does not read (ppdu.rs: IE 1 only `if rate >= OFDM6 && to_self`). For those `rpl` stays 0, and the conversion moves each path's RSSI by `rpl − average`, wrapping as a u8 as `__rtw8852bx_convert_rpl_to_rssi` does, so they land near 0 (−110 dBm) or near 255 (above −20 dBm).
  - This is read from our code. How Linux treats the same reports is not checked.
  - Those are exactly the other stations' OFDM PPDUs that the OFDM PD floor acts on. So the histogram as it stands does not show their signal. A fix is to bin, for a report with no RPL, the paths' RSSI uncorrected.
- **What busy air does to us that it does not do to Linux:** the answer path (213-DRV-0091, the six steps). One frame in flight, a false loss at 90 ms, and every frame behind refused Busy and dropped. Linux queues and aggregates, so a hold only delays.

**The message to NET** (DRV, 14:31 UTC, on `internal-dev-messages`, `NET/2026-11-22T1440Z-DRV-on-a-busy-channel-…`) asks three steps:
1. busy is not a failed frame;
2. a `pending` outcome;
3. an asynchronous window.

The driver's side of whatever NET answers is the successor's.

**The alternation and its paired report:**
- With both switches true, each minute after the first whole one holds either the values found (OFF) or Linux's (ON), turn and turn about, labelled in its floor line.
- The paired report: each ON minute against the mean of its two OFF neighbours, in the same boot, so that the hour and the start cancel. Compare:
  - late frames per send (from the TRAFFIC line's SENT and LATE, as minute differences);
  - the late frames' median hold (the LATE FRAMES RELEASED line);
  - PPDU a minute;
  - the faint share (bins 13 to 15). That share is good only once the end bins are mended, as above.
- Boot 93 is the series: one long boot, so its minutes alternate for about an hour.

**Open:**
- the two end bins (above), and the faint share of other stations' OFDM;
- why late frames do not follow PPDU alone (boot 93's first minute);
- the poll cadence: about 400 gaps over 5 ms a minute, longest 18 to 21 ms, in good and bad boots alike;
- the silent recoveries: SER_DBG_INFO steps with no code and no halt bit, about 2 an hour idle and more under load;
- the unreadable CMAC window with no SER step (boots 81 and 83);
- 5 GHz, which waits for NET (213-DRV-0087, 550-DRV-0088).

## Related

[213-DRV-0091](213-DRV-0091-the-firmware-told-of-the-link.md) (the held frames, the starts series), [213-DRV-0090](213-DRV-0090-the-receiver-goes-deaf-on-a-busy-channel.md) (the busy channel), [213-DRV-0032](213-DRV-0032-rtl8852be-wifi.md) (the driver; DIG, EDCCA and AIFS left).
