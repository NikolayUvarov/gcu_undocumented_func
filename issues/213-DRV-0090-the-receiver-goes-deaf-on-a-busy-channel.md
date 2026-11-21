# 213-DRV-0090 — The RTL8852BE's receiver goes deaf on a busy 2.4 GHz channel

**Type:** driver (`rtw89_wifi`) · **Owner:** `DRV` · **Priority:** P1 · **Status:** open (2026-11-21) · **Blocked by:** — · **Main task:** [213](213-amd-ryzen-pc.md) · **Constitution:** MC-6.1, MC-12.1, MC-12.3

## Problem

On the Ryzen, joined to CoolNet2 on channel 8 (about 650 frames of other stations a second there in the evening), the driver's reception stops for seconds with nobody touching it, and transmission with it. Seen on 2026-11-21 (release 202611211819; the raw lines are in the owner session's handoff directory, `wifi/net-lines-37.txt` and `mon/run1-failed-ring.log`):

- **376.0–382.2 s, the head of 213-DRV-0087:** a 6 MB download received nothing for 6173 ms (`TCP SOCKET 2: BYTES AGAIN AFTER 6173 MS`), and then went on by itself. 6.2 s is a LAN sender's fifth retransmission (0.2 + 0.4 + 0.8 + 1.6 + 3.2 s): the station heard nothing for 3 to 6 s.
- **417.9–430.5 s, the same driver:** not silence but heavy loss. Effector's socket waited 5.9 s and 6.7 s while a download went on at about 80 KB/s, with 58 to 125 retransmissions and a 1.3 s round trip.
- **902.9–912.9 s, the boot image's driver (863735122), during a 30 MB update through Effector:**
  - 167 sends had outrun their time in the 224 s before.
  - The last frame was tried 8 times and not acknowledged; its release came 1921 ms later.
  - From 902.9 s nothing was received at all, until `RECEIVE STALLED ON CHANNEL 8: NO FRAME FOR 10003 MS` stopped the data mode.
  - The update failed: Effector's agent gives up after 10 s without a byte.
  - The fresh driver's scan and join took 7.5 s more.

## What the logs said at the holes (2026-11-21)

- **No driver line at any of them:**
  - no firmware recovery (OUTLAST_SER's line) and no coexistence failure;
  - no `UNHEARD` run of unacknowledged sends;
  - no scan or roam by `wlan`.

  Around 376 s nothing at all: the late sends before and after were at 298.6 and 388.1 s.
- **The sends that outran their time run all through the run, not only at holes.** They are mostly 88- and 100-byte frames, the station's TCP acknowledgements, `FETCHED AND NOT RELEASED AFTER ~88 MS`, and released 100–600 ms after submission. Most are acknowledged at the first try. The hardware holds them before trying, as it does when it sees the channel taken.
- **Transmit dies with receive:** in the stall, a frame was tried 8 times over 1.9 s, not acknowledged, as reception stopped.
- **No period:** the silences began 23.8 s, 246 s and 381 s after their admissions.
- **0xC800 (R_AX_RX_ERR_FLAG) reads C001 at admission and C201 from 1.4 s after it,** in every run. Bit 9 is `B_AX_RX_RXSTS_ZERO_LEN_ERR`, a zero-length receive status, latched early and not at a hole. It is to be looked at beside the PPDU status reports once the holes are characterised.
- **Bluetooth:** `bt_hci` says `NOT LISTENING` this boot. The Bluetooth controller's firmware and the coexistence are still there.
- **What the logs could not tell:** a deaf receiver from quiet air. The driver logged no receive-side counts, no ring indexes, no RF lock and no coexistence grant.

## Plan

1. **Instrument** (this branch): count the joined BSS's beacons where the ring gives them. A hole is a second without one, once one was heard: beacons come about ten a second, so a hole tells "our access point unheard" even while other frames still come.
   - Log each hole's start and end with the counts since the other end, and the registers that can tell a deaf radio from quiet air.
   - Log one line a minute with the counts.
   - Log the same registers on the first minute and every tenth, for a good link's.
2. **Measure on the Ryzen,** with the instrumented driver in the boot image so that every run (reboots, update downloads) collects holes.
3. **Find the cause from them,** against what Linux's 2 s track work does that this driver does not (below).
4. **A shorter hole,** proposed first and landed only after it is agreed: detection in 1–2 s, and a recovery short of the exit, scan and join.

## Acceptance criteria

- [ ] Host cases: a hole opens after a second without the joined BSS's beacon once one was heard, and closes with the next one, with its counts and registers; the minute's line; nothing for a station that heard no beacon yet.
- [ ] On the Ryzen, holes logged with their registers, and their cause named or narrowed with evidence (MC-12.1: that machine, that channel and its hours).
- [ ] A proposal for a shorter hole agreed, and if landed, measured.

## The instrumentation (2026-11-21, host cases; not yet run on the Ryzen)

- **Counting the beacons:** the data mode counts the joined BSS's beacons where the receive ring gives them (`Dispatcher`), whether or not the station reads them. The data service sees the count move at each step.
  - A hole opens after HOLE_MS (1000 ms) without one, once one was heard since the station's filter.
  - It closes with the next one; its length runs from the beacon before to the beacon after.
  - A filter of another BSS starts the count anew.
- **Lines (data mode only):**
  - `[RTW89] HOLE n ON c: NO BEACON OF <bssid> FOR t MS; SINCE IT w FRAMES, k KEPT, r PPDU, p INBOX FULL; s SENT, a ACKED[, ONE PENDING]`, at its start. The frames are every Wi-Fi frame the ring gave, so frames still coming with no beacon mean "our access point unheard", and none means "the receiver hears nothing". The PPDU status reports come for every PPDU the PHY decoded.
  - `[RTW89] HOLE n ENDED: ITS BEACON AFTER t MS; MEANWHILE …`, with the same counts.
  - `[RTW89] TRAFFIC ON c: …` every minute: the frames received, kept, filtered, invalid and dropped with the inbox full; the PPDU reports and the BSS's beacons; the sends answered, acknowledged and late; the holes and the longest.
  - The registers follow the first 12 holes' two lines, and the minute's line on its first minute and every tenth, as `HOLE START n`, `HOLE END n` and `GOOD LINK n` pieces of 150 bytes.
- **The registers** (`data_mode::PROBE_*`, reads only, but the LTE window's indirect read, which the coexistence's reconcile makes too):
  - **MAC:**
    - SYS_FUNC_EN (in 0x0), the coexistence control byte (0x73), the scoreboard (0xAC);
    - HALT_H2C/C2H and their controls, WCPU_FW_CTRL (the firmware's status);
    - the RXQ and RPQ rings' indexes (0x1050, 0x1054: host in bits 0–11, hardware in 16–27; equal: nothing was fed; apart: fed and not read);
    - SER_DBG_INFO, DLE_EMPTY0/1, DMAC_ERR_ISR;
    - MACID pause and sleep, CTN_TXEN and CTN_DRV_TXEN (transmit paused);
    - CMAC_ERR_ISR, RX_ERR_FLAG, RX_FLTR_OPT;
    - the pollution 0xC67C;
    - Linux's coexistence monitor set for this chip (`rtw89_btc_8852b_mon_reg`: 0xCEF4, 0xD200, 0xD220, 0xDA10–0xDA4C);
    - DMA_STOP.
  - **BB:**
    - the same monitor set's 0x980, 0x4738, 0x4688, 0x4694;
    - DIG's packet-detection lower bounds (0x4860, 0x4B64) and initial LNA, TIA and RXB gains (0x472C, 0x4A80, 0x473C, 0x46A8);
    - the EDCCA level (0x4884) and its reports (0x1738, 0x173C);
    - TX collision (0xC70);
    - CCK (0x700, 0x2344).
  - **RF, both paths:** mode (0x00, bits 16–19), channel (0x18), VCO (0xB2), and the synthesizer's lock (0xC5, bit 15: 0 is unlocked, which Linux's `_lck_check` repairs at a channel set).
  - **The LTE window's SW_CFG_1** (`LTE38`): the grants of WLAN and Bluetooth as the coexistence wrote them.
- **What it costs:**
  - **For each frame the filter keeps:** one byte and six compared.
  - **Each step:** a few comparisons.
  - **The minute's line:** one counts read.
  - **The registers:** 60 reads, at the first 12 holes' ends and every tenth minute.
- **Host case:** `tests/rtw89_service_cases.rs`, `service_a_missed_beacon_opens_a_hole_and_the_next_one_closes_it`:
  - no hole before the first beacon;
  - a hole after 1.2 s of frames to the station without one, with its counts and registers (the scheduler as admitted, the ring read to its end, RF 0x18 on channel 6);
  - its end at the next beacon;
  - the minute's line with the hole's length and a good link's registers.

  The 40 `rtw89_*_host.rs` files pass.

## What Linux's 2 s track work does that this driver does not

Linux runs `rtw89_track_work` (`core.c`) every 2 s while associated. This driver runs none of it. In its order:

- **`rtw89_btc_ntfy_wl_sta`, when the traffic level changes:** tells the coexistence WLAN's traffic and RSSI, by which the firmware chooses its time slots and antenna. This driver tells the coexistence of the connection once, at admission.
- **`rtw89_core_bcn_track`, `rtw89_phy_stat_track`, `rtw89_phy_env_monitor_track`:** beacon timing (for power save), RSSI averages, and the CCX measurements DIG and EDCCA use.
- **`rtw89_phy_dig`:** sets the initial gain and the packet-detection lower bound from the RSSI and the false alarms. Without it they stay as the BB tables leave them, and on a crowded channel the receiver starts on every weak preamble of other stations.
- **`rtw89_core_rfk_track` → `rtw8852b_dpk_track`:** DPK's thermal tracking (transmit quality).
- **`rtw89_phy_ra_update`, `rtw89_phy_cfo_track`:** the rate table, and the crystal trimmed to the access point's carrier offset as the temperature moves.
- **`rtw89_phy_edcca_track`:** the energy-detect threshold from the RSSI. A threshold too low holds sends while the channel is taken, which is what the late sends look like.
- **The rest** (path diversity, antenna diversity, UL-TB, SAR, channel contexts, rfkill, MLO, power save) does not bear on this.

**Which of them, missing, could make the receiver deaf for seconds:** none of DIG, EDCCA or CFO explains total silence with frames of 14 networks on the air. They explain loss and held sends. A receiver that hears nothing at all, and a transmitter whose 8 tries go unacknowledged in the same seconds, fit better:

- **the coexistence giving the antenna to Bluetooth** for seconds. This morning's experiment, BTG enabled after the calibration, gave three stalls in four minutes. The probe's `LTE38`, the 0xDA2x monitor registers, 0x73 and the scoreboard tell it;
- **the synthesizer losing lock** (RF 0xC5 bit 15);
- **the MAC's receive path stuck** (RX_ERR_FLAG, DLE_EMPTY, the ring indexes).

The holes' registers are to decide, not this list. This morning's TSSI-tracking experiment (more firmware recoveries) points at the PA and thermal path too: the firmware's recoveries are logged, and none was at these holes.

## A shorter hole: a proposal, not landed

- **Detection:** the hole's line comes 1 s after the last beacon instead of 10 s. 2 s without a beacon and without any frame at all is a deaf receiver, by the counts above.
- **Recovery short of the exit,** to be tried in that order and measured, each only if the holes' registers point at it:
  1. If the grants show Bluetooth holding the antenna: the coexistence's WLAN-only antenna again (`Antenna::WlanOnly`, as the admission sets it).
  2. If RF 0xC5 shows the synthesizer unlocked: Linux's `_lck_check` repair (MMD reset, RF 0x18 written again, the synthesizer off and on), which the channel stage already has for a tune.
  3. Otherwise a re-tune of the same channel (`channel::run`, a few milliseconds, the calibration's results kept in their registers), or a BB reset. Whether IQK or DPK must follow is to be measured.
  4. Only after QUIET_MS, today's exit.
- **The 7.5 s after the exit** are mostly `wlan`'s 5.7 s scan of a restarted radio, which dwells 150 ms on each of the 28 barred 5 GHz channels too. NET is asked for two things, in `NET/2026-11-21T1856Z-…`: the rejoin of the known BSS on its known channel before any scan, and a scan that skips barred channels.

## Related

[213-DRV-0032](213-DRV-0032-rtl8852be-wifi.md) (the driver), [801-DRV-0071](801-DRV-0071-the-bench-radios-keep-their-network-across-releases.md) (the stalled receive's stop), [213-DRV-0052](213-DRV-0052-coexistence-after-association.md) (coexistence after association), [213-DRV-0087](213-DRV-0087-rtl8852be-on-5-ghz.md) (this branch's base).
