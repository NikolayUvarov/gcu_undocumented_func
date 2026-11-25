# 213-DRV-0091 — The RTL8852BE's firmware told of the station's link

**Type:** driver (`rtw89_wifi`) · **Owner:** `DRV` · **Priority:** P1 · **Status:** open (2026-11-21) · **Blocked by:** — · **Main task:** [213](213-amd-ryzen-pc.md) · **Constitution:** MC-6.1, MC-12.1, MC-12.3

## Problem

Since its first day, this driver has told the firmware and the MAC that the station has no link, even while it is joined and carrying traffic. On 2026-11-21 the words it sends were checked bit by bit against the pinned Linux sources (commit 05f7e89a: `fw.h`, `fw.c`, `core.h`, `core.c`, `mac.c`, and `cam.h`, `cam.c`, `mac80211.c` of the same commit).

- **Join info** (`rtw89_fw_h2c_join_info`, fw.c; its layout fw.h's `RTW89_H2C_JOININFO_W0_*`): the station stage sends `0x04000100`:
  - MACID 0;
  - OP (bit 8) 1 = `dis_conn`;
  - NET_TYPE (25:24) 0 = `RTW89_NET_TYPE_NO_LINK` (core.h `enum rtw89_net_type`);
  - WIFI_ROLE (29:26) 1 = STATION;
  - SELF_ROLE (31:30) 0 = CLIENT.

  That is Linux's add-interface call (mac.c, `rtw89_mac_vif_init`: `rtw89_fw_h2c_join_info(rtwdev, rtwvif_link, NULL, true)`). Linux sends it again at association from `rtw89_core_sta_link_assoc` (core.c) with `dis_conn` false and NET_TYPE INFRA, which `rtw89_vif_type_mapping(link, true)` (core.c) has set. This driver never sent that one.
- **Role maintain** (`rtw89_fw_h2c_role_maintain`, fw.c; fw.h's `RTW89_H2C_ROLE_MAINTAIN_W0_*`): `0x2000`, which is MACID 0, SELF_ROLE CLIENT, UPD_MODE CREATE, WIFI_ROLE STATION. That is Linux's role creation (mac.c, `rtw89_mac_vif_init`), and it is right.
- **Address CAM** (`rtw89_cam_fill_addr_cam_info`, cam.c; cam.h's `ADDR_CAM_W*`): VALID, NET_TYPE (w2 2:1) 0 = NO_LINK, AID12 (w9 11:0) 0, the station's MAC and the BSSID, a valid BSSID entry with mask 0x3F. At association Linux sends `rtw89_fw_h2c_cam(..., RTW89_ROLE_CON_DISCONN)` (core.c, "update cam aid mac_id net_type") with NET_TYPE INFRA and the association's AID. The filter change in data mode (`station_update.rs`) sent the same NO_LINK entry again.
- **Port 0** (`rtw89_mac_port_update`, mac.c; at association from mac80211.c's `__rtw89_ops_bss_link_assoc`): left at NO_LINK, with PORT_FUNC_EN alone (`station.rs`'s `verify` wants mask 0x13E1F to read 4):
  - net type 0;
  - no RX_BSSID_FIT_EN;
  - no TSF_UDT_EN;
  - no TBTT_PROHIB_EN or BRK_SETUP.
- **What goes with it on the Ryzen** (213-DRV-0090): no beacon of any network reaches the host in data mode, sends are held 88 ms and more before their first try, and the receiver goes quiet for seconds.

  Whether a firmware told "no link" causes any of that is what this task measures. It is not a known cause.

## Plan

One change and one measurement at a time.

1. **Step 1, the two H2Cs** (this branch):
   - When the data service receives its access point's (re)association response to this station with status 0, it sends:
     - the join info connected (OP 0, NET_TYPE INFRA, WIFI_ROLE STATION, SELF_ROLE CLIENT: `0x06000000`);
     - then the address CAM with NET_TYPE INFRA and that AID, in Linux's order.
   - The access point's deauthentication or disassociation of this station, and a new filter, send the NO_LINK pair again.
   - The port is not touched.
   - Measured on the Ryzen against the boot image's driver, back to back:
     - the minute line's BEACONS;
     - MGNT_FLTR and PORT_CFG before and after, from 213-DRV-0090's probe;
     - late sends a minute;
     - a 6 MB download's rate;
     - holes and `RECEIVE STALLED` in 15 minutes of traffic.
2. **Step 2, the port update to INFRA** (`rtw89_mac_port_update` with net type INFRA): net type 2, RX_BSSID_FIT_EN, TSF_UDT_EN, TBTT_PROHIB_EN, BRK_SETUP and the access point's beacon interval. The station check's expected PORT value moves with it. Then the same measurement.
3. **What else Linux sends at association** (below): each marked, so that the next gap is not found by accident.

## Acceptance criteria

- [ ] Host cases: an association response with status 0 sends the connected join info and the INFRA CAM with its AID; a refused one sends nothing; a deauthentication and a new filter send the NO_LINK pair.
- [ ] Step 1 measured on the Ryzen against the boot image's driver (MC-12.1: that machine, that channel, that hour).
- [ ] Step 2 written, host cases, measured the same way.
- [ ] The list below kept up to date.

## What Linux's association sends or sets that this driver does not

From `rtw89_core_sta_link_assoc` (core.c) and `__rtw89_ops_bss_link_assoc` (mac80211.c):

| Item | Linux | Here | Mark |
|---|---|---|---|
| Join info, connected (`rtw89_fw_h2c_join_info`, dis_conn false) | at association | step 1 | needed: this task |
| Address CAM with NET_TYPE INFRA and AID (`rtw89_fw_h2c_cam`, CON_DISCONN) | at association | step 1 | needed: this task |
| Port update for INFRA (`rtw89_mac_port_update`) | at association | step 2 | needed: this task |
| Association CMAC table (`rtw89_chip_h2c_assoc_cmac_tbl`) | at association | CCTL at the station stage only | unknown: to compare with the CCTL this driver sends |
| Rate adaptation (`rtw89_phy_ra_assoc`, its RA H2C) | at association | none; the station chooses each send's rate | not needed with host rate control, as long as sends carry their rate |
| Beamforming (`rtw89_mac_bf_assoc`) | at association | none | not needed on 11g |
| BTC role notice, STA_CONN_END (`rtw89_btc_ntfy_role_info`) | at association | a connection notice at admission | unknown: what the admission's notice says against Linux's at association |
| UL TB, HE HTC template (`rtw89_phy_ul_tb_assoc`, `rtw89_core_get_no_ul_ofdma_htc`) | at association | none | not needed on 11g |
| Beacon tracking (`rtw89_core_bcn_track_assoc`) | at association | none | needed later, with beacons on the host (power save, TSF) |
| General packets (`rtw89_fw_h2c_general_pkt`: null data, QoS null, PS-poll templates) | at association | none | needed later, for power save and for the firmware's own keep-alive |
| Beacon filter offload (`rtw89_fw_h2c_set_bcn_fltr_cfg`) | if the firmware has `BEACON_FILTER` | none | unknown: whether this firmware's feature table has it |
| DIG resume (`rtw89_phy_dig_resume`) | at association | no DIG at all | needed later (213-DRV-0090's list of the track work) |
| BSS colour, HE OBSS narrow RU, HE TB (`rtw89_phy_set_bss_color`, `rtw89_mac_set_he_obss_narrow_bw_ru`, `rtw89_mac_set_he_tb`) | at association | none | not needed on 11g |
| UL TB power offset (`rtw89_chip_cfg_txpwr_ul_tb_offset`) | at association | none | not needed on 11g |

## Progress

**2026-11-21, step 1 written** (host cases; not yet run on the Ryzen):
- **`station.rs`:**
  - `join_info(connected)` gives the join info word, citing fw.h's layout;
  - `cam_for(mac, bssid, aid)` gives the address CAM, citing cam.h's layout, with NET_TYPE INFRA and AID12 when connected;
  - the station stage sends `join_info(false)`, the same word as before.
- **`station_update.rs`:** `Prepared::connect(update, aid)` sends join info, then the CAM, and checks the station's registers.
- **`active_control.rs`:** `DataSession::connect(aid, timeout)` runs it under the data mode's own admission, like a filter change: no send pending, live checks, coexistence reconcile.
- **`service.rs`:**
  - collect() takes the access point's (re)association response to this station with status 0 and its AID, or its deauthentication or disassociation;
  - the next step with no send pending tells the firmware;
  - a new filter tells it "no link" first.
  - Lines: `[RTW89] THE FIRMWARE TOLD: CONNECTED TO <bssid> ON CHANNEL c AS AID n (JOIN INFO AND ADDRESS CAM AS INFRA; 213-DRV-0091)` and `… NO LINK ON CHANNEL c (WAS AID n; …)`.
- **Host case:** `tests/rtw89_service_cases.rs`, `service_an_association_response_tells_the_firmware_of_the_link`:
  - a refused association (status 17) sends nothing;
  - status 0 and AID 1 send the join info `00 00 00 06` and the CAM with NET_TYPE 2, AID 1 and the BSSID;
  - the deauthentication sends `00 01 00 04` and a NO_LINK CAM;
  - a reassociation and then a new filter send "no link" before the filter's own CAM.

  The 40 `rtw89_*_host.rs` files pass.

**2026-11-21, 20:13–20:16 UTC, step 1's first look on the Ryzen** (b3db40619 by `replace` into release 202611211959, channel 8, country UZ; the boot image's driver back after):
- **The link line:** `513.032 … THE FIRMWARE TOLD: CONNECTED TO A0:36:BC:11:F8:88 ON CHANNEL 8 AS AID 31`, 0.22 s after the admission and just before `wlan`'s `JOINED`.
- **The BSS's beacons on the host:** 580 in the first minute and 1160 after two, which is its whole rate of about 9.7 a second. Every data-mode minute line before this showed 0 (213-DRV-0090, boots 55 to 59 and a bench run). Beacons of other networks: 0.
- **Sends that outran their time:** 1 in the first minute, 4 after two, of 1493 answered. The boot image's driver in boot 55 had 131 of 7948, about 1.6%; these are 0.27%. One short run, so this is not yet a claim.
- **One 6 MB download:** 255.5 KB/s by the sender, 233 KiB/s by the stack (`QUEUE FULL AT 0 OF 1738 READS`), within the 241–265 KB/s of the evening's earlier runs. No gain in rate is shown.
- **No hole and no stall in the two minutes.**
- **The registers just before and after the two H2Cs** (`LINK BEFORE`/`LINK AFTER`):
  - the type filters read 55555555 both times (every subtype to the host);
  - PORT_CFG reads C004 both times (FUNC_EN, BCN_DROP_ALLOW, TBTT_UPD_SHIFT_SEL; the port is untouched in step 1).

  So no host-side filter register held the beacons back. What changed is the firmware's and the MAC's view of the link: the join info and the CAM's NET_TYPE.
- **The frames the station's filter refuses** are other networks' group data, not beacons:
  - STP (01:80:C2:00:00:00) from ZATV, A0:36:BC:11:F8:89;
  - IP multicast (01:00:5E…) from EC:43:F6:D9:D7:A8.
- **Next:** the back-to-back measurement against the boot image's driver (three downloads each way, 15 minutes of traffic), then step 2.

**2026-11-21, 20:23–20:31 UTC, mirrored back to back on the Ryzen.** b3db40619 is the boot image's driver since the night release 202611212016; the old driver is 1c83d581e by `replace`. Channel 8, UZ, three 6 MB downloads each, by the sender's figures:

| Driver | 6 MB downloads | BSS beacons a minute | Sends outrun a minute | Holes |
|---|---|---|---|---|
| b3db40619 (boot image) | 246, 285, 265 KB/s | 581 | 1 to 4 | none |
| 1c83d581e (replace) | 300, 342, 223 KB/s | 0 | 0 in 2 minutes | 5 by frames, idle 1.0–1.3 s |
| b3db40619 (boot image again) | 292, 259, 296 KB/s | 581 | 1 to 3 | none |

- **The rate:** no difference is shown either way; the spread within each set is larger than between them.
- **The beacons:** they reach the host with step 1 and not without it, on every boot and run so far (boot 61: 581 in its first minute, AID 23).
- **Sends that outran their time:** this evening the old driver had almost none too (9 in boot 60's 15 minutes of a 30 MB download). The lower count of 20:13–20:16 was the hour, not the change; nothing is claimed about them.
- **Tonight's test:** the boot image's driver runs through the night, with Effector's traffic. Its boot logs are read for the beacons, late sends, holes, stalls and anything new from the firmware now that it knows the link.

**2026-11-21, step 2 written** (branch `claude/213-DRV-0091-step-2-the-port`; host cases; not yet run on the Ryzen):
- **`station.rs`, `port_link(bus, linked)`:** Linux's `rtw89_mac_port_update` (mac.c) for a port whose function is on:
  - `rtw89_mac_port_cfg_func_sw` turns PORT_FUNC_EN and BRK_SETUP off, sets TSFTR_RST and writes BCN_CNT_TMR 0;
  - then PORT_CFG's link fields for INFRA: NET_TYPE 2 (`rtw89_mac_port_cfg_net_type`), TBTT_PROHIB_EN and BRK_SETUP (`…_bcn_prct`), RX_BSSID_FIT_EN (`…_rx_sw`) and TSF_UDT_EN (`…_rx_sync_by_nettype`), or none of them for NO_LINK;
  - PORT_FUNC_EN on (`…_func_en`), 20 µs (`BCN_ERLY_SET_DLY`), the beacon-early time 160 (`…_bcn_early`) and BCN_PSR_RPT's BSSID index 0 (`…_bcn_psr_rpt`).
- **The beacon timing the station stage already writes** (setup, hold, mask area, the interval at 100 TU, TBTT early, HIQ, MBSSID) is the same for INFRA and NO_LINK in Linux, and is not written again. Linux's interval is the BSS's `beacon_int`; this driver keeps Linux's default `BCN_INTERVAL` 100. The bench access point's own interval is not read yet.
- **Linux's wait of a beacon interval and a millisecond before turning the port off** (`msleep(beacon_int + 1)`) is the data service's: `PORT_WAIT_MS` 101 after the link's H2Cs, then the port update as its own operation when no send is pending. On a new filter, the NO_LINK port follows the NO_LINK H2Cs at once.
- **The station's register checks** (`verify`, run at every data-mode check) expect PORT_CFG to read `0x1281C` under mask 0x13E1F once linked, and 4 otherwise.
- **Line:** `[RTW89] PORT 0 SET FOR INFRA|NO LINK ON CHANNEL c (…; 213-DRV-0091)`, with the registers after it (`PORT AFTER n`) the first times.
- **Host case:** `service_the_link_sets_port_0_for_infra_after_a_beacon_interval`:
  - the H2Cs first, with the port untouched;
  - nothing at 50 ms;
  - at 110 ms PORT_CFG reads `1281C`;
  - steps with traffic run on under the new expectation;
  - the deauthentication brings it back to 4.

  The 40 `rtw89_*_host.rs` files pass. Its hardware run comes after step 1's night, measured as step 1 was.

**2026-11-22, boot 62: the data mode stopped twice on an unreadable CMAC register** (98 minutes, the boot image's driver):
- **The stops:** at 1003.959 s and 3248.465 s. Each was `Station(CheckFailure { cause: Fault { kind: PowerState, register: 0x40018, value: DEADBEEF } })`: the station check read the CMAC window and got DEADBEEF. A fresh driver started each time.
- **The registers at the stops:**
  - HALT_H2C_CTRL (0x160), HALT_C2H_CTRL (0x164) and HALT_C2H (0x16C) read 0;
  - SER_DBG_INFO read F0000001 at the first stop and F0000002 at the second;
  - CMAC_ERR_ISR read 0.
- **Between the stops,** the third driver logged `FIRMWARE RECOVERY 1 BETWEEN OPERATIONS (SER_DBG_INFO F0000001, CMAC_ERR_ISR 00000040)`. BB and the scheduler were back after 509 µs and the data mode went on.

  CMAC_ERR_ISR bit 6 is B_AX_WMAC_RX_ERR_IND (reg.h). So the firmware recovers from a receive error, and a station check that happens to fall inside the recovery reads DEADBEEF and stops the data mode.

**2026-11-22, the check mended** (branch `claude/213-DRV-0091-the-check-mended`; host cases, not yet run on the Ryzen):
- **`data_mode.rs`, `live()`:** a station check whose first failure reads DEADBEEF first takes the path a recovery between operations already takes: if SER_DBG_INFO moved, it waits for BB and the scheduler. Then `read_again()` checks again 2 ms apart, up to 8 reads in all, and outlasts a recovery whenever SER_DBG_INFO moves meanwhile.
- **When it stops:** only on a mismatch that persists with a readable value, or on a register still DEADBEEF after the 8 reads. The CMAC table check is kept.
- **The line for each meeting:**

  ```
  [RTW89] CHECK MET A FIRMWARE RECOVERY n: <reg> READ DEADBEEF, READABLE AGAIN AFTER r READS IN t US; SER_DBG_INFO a -> b (213-DRV-0091)
  ```

  It says `AN UNREADABLE CMAC` in place of `A FIRMWARE RECOVERY` when no recovery was seen. n counts every meeting.
- **The probe of a hole or a stop** also reads the error masks: DMAC_ERR_IMR 0x8520, CMAC_ERR_IMR 0xC160 and RX_ERR_FLAG_IMR 0xC804.
- **Host case:** `service_a_check_that_meets_an_unreadable_cmac_window_reads_again_and_goes_on` (tests/rtw89_service_cases.rs). The model's CMAC window can read DEADBEEF for a set number of reads, with or without a recovery started. The case covers:
  - the check first, with no recovery: meeting 1, `recovered` false;
  - the check first, the recovery starting during it: meeting 2, `recovered` true, SER_DBG_INFO changed;
  - the recovery first, then the check: the recovery is taken, then meeting 3;
  - traffic going on after each;
  - a window that stays unreadable for 1000 reads: the service stops with a radio failure.

  The 40 `rtw89_*_host.rs` files pass, and the release build builds.

**2026-11-22, why the firmware recovers: the error masks and the host's part in a recovery** (reading only; Linux 05f7e89's mac.c, ser.c, reg.h and rtw8852b.c):
- **The masks are the same as Linux's for the 8852B.**
  - `post_mac.rs` writes the DMAC and CMAC tables of Linux's `enable_imr_ax` with the 8852B's `imr_info` values (the `MASKS` table, from `post_mac_regs.rs`).
  - It writes `err_imr_ctrl`'s DMAC_ERR_IMR and CMAC_ERR_IMR as all ones.
  - Linux never writes R_AX_RX_ERR_FLAG_IMR (0xC804) for the 8852B; mac.c only prints it in its error dump.
  - What the masks read in data mode is not yet known: the probe reads them from this branch on.
- **What a recovery needs from the host** (ser.c):
  - Linux takes the firmware's error from HALT_C2H. `rtw89_mac_get_err_status` waits for HALT_C2H_CTRL to be nonzero, reads HALT_C2H and clears HALT_C2H_CTRL; ser.c's `rtw89_ser_notify` turns the code into an event.
  - **L0** (ERR_CMAC0/1, L0_RESET_DONE) becomes SER_EV_L0_RESET, which no state of ser.c acts on: the firmware recovers alone.
  - **L1** (L1_ERR_DMAC, L0_PROMOTE_TO_L1) is a handshake. The host:
    1. stops TX, DMA and RX and resets the rings;
    2. answers M2 through HALT_H2C;
    3. waits for M3;
    4. answers M4;
    5. waits for M5.

    Each wait has a 1 s alarm, and a timeout goes to L2.
  - **This driver** never reads or clears HALT_C2H and has no L1 handshake. A recovery between operations only waits for BB and the scheduler.
  - **What boot 62 shows:** both stops read HALT_C2H_CTRL and HALT_C2H as 0, and the recovery between them was over in 509 µs with no host action. That fits an L0 recovery, which Linux would also leave to the firmware. With only two readings, this is not established.
- **Which to fix first:** the host's part, by its first step only.
  - The change: read HALT_C2H_CTRL at each data-mode check; when it is nonzero, read HALT_C2H, clear the CTRL as Linux does, and log the code and the level.
  - The measurement: one night. It says whether L1 requests come and go unanswered, which would need the handshake, or the recoveries are L0 only.
  - The masks match Linux, so changing them would only move away from the reference without a reason.

**2026-11-22, the firmware's error codes read** (branch `claude/213-DRV-0091-the-firmwares-error-codes`, on the mended check; host cases, not yet run on the Ryzen):
- **`data_mode.rs`, `firmware_status()`,** runs at the start of each data-mode check (`live()`). It reads HALT_C2H_CTRL (0x164). When that is nonzero and not all ones, it:
  - reads HALT_C2H (0x16C) and SER_DBG_INFO;
  - clears HALT_C2H_CTRL, as `rtw89_mac_get_err_status` does (mac.c);
  - counts the code by its level and keeps up to 4 for the service's lines.

  Nothing is answered to the firmware: no M2 or M4 is sent, and HALT_H2C and its control are not written.
- **`firmware_error()`** names the code as mac.h's `mac_ax_err_info` does, after mac.c's scenario mapping (bits 31:28 of 1, 2 and 3 become RXI300, CPU_EXCEPTION and ASSERTION). It gives the level and the event ser.c's `rtw89_ser_notify` makes of it:
  - L0: ERR_CMAC0/1 and L0_RESET_DONE, which become SER_EV_L0_RESET;
  - L1: L0_PROMOTE_TO_L1, L1_PREERR_DMAC, L1_ERR_DMAC, L1_RESET_DISABLE_DMAC_DONE and L1_RESET_RECOVERY_DONE, the steps pre-M0, M1, M3 and M5;
  - L2: L1_PROMOTE_TO_L2 and 0x2000 to MAC_AX_GET_ERR_MAX;
  - L1_RCVY_STOP_DONE is in mac.h's L1 group, but ser.c makes no event of it;
  - anything else is UNKNOWN.
- **The lines:**

  ```
  [RTW89] THE FIRMWARE'S ERROR CODE n: HALT_C2H <raw> (CTRL <ctrl>) = MAC_AX_ERR_<name> <code>, <level> (<ser.c event>); SER_DBG_INFO <v>; NOT ANSWERED (213-DRV-0091)
  ```

  Each minute also has `[RTW89] TRAFFIC ON c: THE FIRMWARE'S ERROR CODES a L0, b L1, c L2, d UNKNOWN (213-DRV-0091)`, counted since the data mode began.
- **What this cannot see:**
  - Linux takes each code on the halt C2H interrupt (pci.c), and this driver only at its checks. If the firmware writes a new code over an unread one, only the last is seen.
  - Codes posted outside the data mode are not read.
- **Host case:** `service_reads_the_firmwares_error_codes_names_their_level_and_answers_nothing` (tests/rtw89_service_cases.rs). The model posts each code in HALT_C2H with the CTRL set once the CTRL reads clear. It posts:
  - an L0 recovery's two codes;
  - an L1_ERR_DMAC;
  - an assertion by its scenario bits;
  - an unknown code.

  Each is read once, in order, with its name and level. Nothing is written to HALT_H2C or its control. The minute's line counts 2 L0, 1 L1, 1 L2 and 1 unknown, and traffic goes on.

  The 40 `rtw89_*_host.rs` files pass, and the release build builds.

**2026-11-22, two questions on the recoveries** (reading only: Linux 05f7e89's ser.c, mac.c, mac.h and pci.c, and the boot logs already fetched):
- **When the firmware raises L1 and the host never answers, what does Linux's code say happens?**
  - **The host's side only.** Each step of the handshake sets a 1000 ms alarm:
    - after pre-M0, `ser_l1_reset_pre_st_hdl` answers with L1_RESET_START_DMAC and waits for M1;
    - after M1, `ser_reset_trx_st_hdl` stops TX, DMA and RX, resets the rings, answers M2 (L1_DISABLE_EN) and waits for M3;
    - after M3, `ser_do_hci_st_hdl` answers M4 (L1_RCVY_EN) and waits for M5.
  - **Any alarm that runs out goes to SER_L2_RESET_ST.** Linux dumps the firmware's memory and backtrace, stops the core and calls `ieee80211_restart_hw`, a full restart of the device by the host. L2_RECFG_TIMEOUT (1000 ms) only logs.
  - **What the firmware does by itself when the host stays silent, Linux's code does not say:** Linux always answers. mac.h has codes by which the firmware reports giving up:
    - L1_PROMOTE_TO_L2 (0x1010), which ser.c turns into L2;
    - L1_RCVY_STOP_DONE (0x1011), of which ser.c makes no event.

    Whether the firmware posts them after a timeout of its own, and how long that is, is not in the code read.
  - **So the night's codes decide.** If an L1 code (0x0999, 0x1000 or 0x0010) is followed by traffic coming back with no further code, the firmware resumed by itself, and the 3–6 s silences would be that. If it is followed by L1_PROMOTE_TO_L2, or by nothing until the stall detector restarts the driver after 10 s, the firmware stays halted. That restart is this driver's L2, about 9 s later than Linux's.
- **What SER_DBG_INFO shows already:**
  - Before any recovery it reads 0. After the n-th it reads F000000n: bits 31:28 are 15 and the low bits count.
  - Bits 31:28 are B_AX_L0_TO_L1_EVENT_MASK (reg.h). mac.c's `rtw89_mac_dump_l0_to_l1` reads them as the cause of an L0 promoted to L1. 15 is mac.h's MAC_AX_L0_TO_L1_EVENT_MAX, not one of the six named causes (0 to 6).
  - What 15 means to the firmware is not in Linux's code.
- **Do the join info and the CAM change what the firmware does on a receive error, or is boot 62's count only its length?**
  - The logs fetched so far: boots 55 to 62, with boot 59 only to 186 s and boot 60 to 902 s.
  - A data mode's time runs from its first minute line less 60 s to its last line. A recovery is a step of SER_DBG_INFO's count, from the probe and stop lines, or a `FIRMWARE RECOVERY` line.

  | Driver | Runs (boot: pid) | Data-mode time | Recoveries | Per hour |
  |---|---|---|---|---|
  | Before step 1 (no join info) | 55: 21, 56: 46, 58: 21, 59: 21, 60: 21 and 51, 61: 52 | 0.66 h | 1 (boot 59, at 157 s) | 1.5 |
  | Step 1 (join info, CAM) | 60: 49, 61: 21 and 57, 62: 21, 49 and 50 | 1.87 h | 4 (all in boot 62) | 2.1 |

  - **A recovery needs no association:** the old driver's boot 59 has one, with the firmware never told of a link.
  - **The rates do not differ by more than the counts can show.** Five recoveries in 2.5 hours are too few to tell apart anything but a large difference. If both drivers had the same rate, the old driver's share of the time would give it about 1.3 of the 5; it has 1.
  - **The traffic also differed.** Boot 62 carried the night's Effector traffic, and most of the old driver's time was in short daytime runs.
  - **Still to read:** the rest of boot 59, boots 63 and 64, and the night's boots. They add old-driver time only from boot 59, so a fair comparison needs a night with the old driver too. No such night is planned.

**2026-11-22, the night on the Ryzen: boots 63 to 65** (read from the log volume through the server, without a lease; channel 8, UZ, A0:36:BC:11:F8:88):
- **The builds:**
  - boot 63: 202611212213, step 1 as b3db40619, 22:18 to 22:37;
  - boot 64: 202611212235, with c6202f1c8, 22:39 to 22:47;
  - boot 65: 202611212244, with 4ae48ce88, from 22:48, 5.3 hours read.
- **What the night did not have:** no hole, no RECEIVE STALLED, no `MEDIUM ERROR`, and no firmware error code. HALT_C2H_CTRL read 0 at every data-mode check, and all 315 minute lines of boot 65 read 0 L0, 0 L1, 0 L2 and 0 unknown.
- **The data mode stopped once in boot 64 and four times in boot 65,** each time with `Register(Fault { kind: Clock, register: 0, value: 2000 })`.
  - **The cause is this branch's own.** `read_again()` waited 2000 µs between reads, but the platform's `io::short_delay` refuses more than 1000 µs, and the value in the fault is that delay.
  - **So the mending did nothing.** Every check that met DEADBEEF, unmended by the recovery path, stopped on its first wait. No `CHECK MET` line was reached.
  - **Why the host cases passed:** the host model's delay had no limit.
  - **Each stop cost the link 5.0 to 7.1 s.** netstack kept its address and connection, and TCP bytes came again after about 8.5 s. One restart, pid 51 at 11497.8 s, failed in the PCIe PHY stage (a Timeout at 0x10A0, value 29); the next started 0.35 s later.
  - **The registers at the five stops:** HALT_C2H_CTRL and HALT_C2H read 0. SER_DBG_INFO read F0000002, F0000001, F0000008, 0, and 0 in boot 64.
- **The firmware still recovers, without a code.**
  - SER_DBG_INFO's count rose in three of boot 65's five runs: to 2, to 1, and from 1 to 8. That is about 10 recoveries in 5.3 hours.
  - None left HALT_C2H_CTRL set at a check, and none was met as a recovery between operations.
  - Each driver finds the halt C2H interrupt enabled by the firmware (HIMR0 0x00200000, B_AX_HALT_C2H_INT_EN) and masks it, unlike Linux, which keeps it.
  - Whether the firmware posts its codes only while that bit is on is not known.
- **The minutes, by run:**

  | Run | Minutes | Beacons a minute, median (min to max) | Late sends a minute, median (max) | Sent / acked |
  |---|---|---|---|---|
  | Boot 63 | 19 | 583 (577–586) | 1 (6) | 7598 / 7570 |
  | Boot 64, pid 49 | 6 | 582.5 (581–585) | 0.5 (2) | 4711 / 4702 |
  | Boot 65, pid 21 | 66 | 583 (544–586) | 1 (26) | 9575 / 9435 |
  | Boot 65, pid 49 | 35 | 583 (571–586) | 0 (6) | 5055 / 5024 |
  | Boot 65, pid 50 | 89 | 583 (578–586) | 0 (3) | 12898 / 12838 |
  | Boot 65, pid 52 | 74 | 584 (579–586) | 0 (4) | 10692 / 10637 |
  | Boot 65, pid 57 | 51, running | 584 (552–586) | 0 (4) | 7944 / 7908 |

  The inbox was never full.

**2026-11-22, the wait mended** (host cases; not yet run on the Ryzen):
- **The wait:** `read_again()` waits 1000 µs between reads, up to 16 reads in all, about the same 15 ms as before.
- **The host model** (`tests/rtw89_pipeline_device.rs`) now refuses a delay over 1000 µs as `io::short_delay` does.
  - Before the change, the unreadable-window case failed under it, as the Ryzen did. Every other case of the 40 files passed.
  - After the change, all 40 pass, and the release build builds.

**2026-11-22, 04:57–05:07 UTC, step 2's first run on the Ryzen, back to back** (release 202611220438, boot 66, its driver 07e59deb9). Step 2 is 611499237, which is 07e59deb9 plus the port alone, put in by `replace`. Channel 8, UZ, three 6 MB downloads each, by the sender's figures:

| Driver | 6 MB downloads | The stack's KiB/s | BSS beacons a minute | Sends outrun a minute | Holes, stops |
|---|---|---|---|---|---|
| Step 2 (replace) | 306, 186, 193 KB/s | 260, 157, 167 | 581, 579, 577 | 0, 5, 20 | none |
| 07e59deb9 (boot image) | 247, 265, 346 KB/s | 214, 245, 249 | 579, 582 | 2, 2 | none |
| Step 2 (replace again) | 347, 340, 312 KB/s | 260, 252, 248 | 576, 577 | 0, 2 | none |

- **The port:**
  - `PORT 0 SET FOR INFRA ON CHANNEL 8` came 0.10 s after `THE FIRMWARE TOLD: CONNECTED`, each time.
  - PORT_CFG (0xC400) read C004 before and after the link's H2Cs, and 1E81C after the port step. Under the check's mask 0x13E1F that is 1281C, the INFRA expectation.
- **The error masks in data mode** (the probe now reads them): DMAC_ERR_IMR 0x8520 and CMAC_ERR_IMR 0xC160 read FFFFFFFF, as written. RX_ERR_FLAG_IMR 0xC804, which neither this driver nor Linux writes, reads 00202800.
- **The rate:** no difference is shown. The spread within the step 2 sets (186 to 347) is larger than between the drivers. Step 2's first set had 20 late sends in its third minute; its second set had 2.
- **The cameras' usb_host replaces** (04:51:25 and 04:55:41, just before this run) left no line from the driver beyond its minute lines.
- **Left running:** the boot image's driver, 07e59deb9.

**2026-11-22, why the driver masks HIMR0's halt bit, and HISR0 read under the mask** (branch `claude/213-DRV-0091-the-halt-status`, on 07e59deb9; host cases, not yet run on the Ryzen):
- **Who masks the bit, and why.** `mac::mask_host` came with 71589b8b9 (213-DRV-0032, 2026-11-04, "the bounded diagnostic passes on the Ryzen").
  - Its message: "the three host interrupt masks cleared, before the MAC preparation and after the download (the firmware enables HIMR0's halt interrupt), as disable_intr does".
  - main.rs's note: "The firmware enables its halt interrupt (HIMR0 bit 21); this polled driver keeps host interrupts masked".
  - 213-DRV-0032's table records it: HIMR0 `00200000` after the download, 0 before.
  - So the reason is that this driver polls and has no interrupt handler. Masking was to follow `rtw89_pci_disable_intr`, the state Linux keeps before it starts. Nothing in the issue or the code says the bit was masked to change the firmware's behaviour.
- **What Linux does with the bit** (pci.c, pci.h and rtw8852be.c at 05f7e89):
  - The 8852BE uses `rtw89_pci_config_intr_mask`, which sets `halt_c2h_intrs = B_AX_HALT_C2H_INT_EN` (bit 21 alone; WDT_TIMEOUT, bit 22, is added only by the `_v1` mask functions of other chips).
  - `rtw89_pci_ops_start` calls `rtw89_pci_enable_intr`, which writes HIMR0 (0x1A0) with it. `rtw89_pci_disable_intr` writes 0 only around stops and while the interrupt is handled.
  - `rtw89_pci_recognize_intrs` reads HISR0 (0x1A4), keeps the bits of `halt_c2h_intrs`, and writes them back to clear them. A halt C2H bit leads to `rtw89_ser_notify(rtw89_mac_get_err_status())`.
  - So Linux runs with bit 21 on, and this driver runs with it off.
- **The change: `firmware_status()` now reads HISR0 first at each check.**
  - When bit 21 (HALT_C2H) or bit 22 (WDT_TIMEOUT) is set, it writes those bits back, as `rtw89_pci_recognize_intrs` does, then reads SER_DBG_INFO and counts them.
  - It then reads HALT_C2H_CTRL as before.
  - Nothing is answered to the firmware, and HIMR0 stays masked.
  - New line: `[RTW89] HISR0 HALT BITS n: <hisr> (HALT_C2H WDT_TIMEOUT) WITH HALT_C2H_CTRL <ctrl>; SER_DBG_INFO <v>; CLEARED AS LINUX DOES, NOTHING ANSWERED (213-DRV-0091)`.
  - The minute line adds `HISR0 HALT_C2H h, WDT_TIMEOUT w`.
- **What it tells.** If the silent recoveries raise HISR0's halt bit with HALT_C2H_CTRL still 0, the firmware signals the host and posts no code. If HISR0 stays clear too, the firmware does not signal the host for them at all, at least under this mask.
- **Host case:** `service_reads_hisr0s_halt_bits_under_the_mask_and_answers_nothing`. The model's HISR0 is cleared by writing its bits back, and a posted code raises bit 21. The case covers:
  - a HALT_C2H bit with no code, then a WDT_TIMEOUT bit, each seen once and cleared;
  - a posted L0_RESET_DONE, with its bit and its code;
  - nothing written to HALT_H2C or its control;
  - the minute line counting 2 HALT_C2H and 1 WDT_TIMEOUT.

  The 40 `rtw89_*_host.rs` files pass, and the release build builds.

**2026-11-22, could HIMR0's halt bit be left on with no interrupt reaching anyone?** (branch `claude/213-DRV-0091-the-halt-interrupt`, on the HISR0 read; host cases, not run)
- **What this driver's code and the kernel's say:**
  - **MSI is not enabled by anyone for this device.** The kernel aims a device's MSI or MSI-X only when init grants a driver `PLATFORM_DEVICE_MSI`/`_MSIX` (kernel/src/pci.rs `msi`, `msix`). rtw89_wifi holds no interrupt capability.
  - **INTx Disable is whatever the firmware left.** The kernel's Command transitions (`pci_control.rs`) change bits 0 to 2 only and keep the rest. rtw89_wifi's configuration access is read only (`SYSCALL_DEVICE_CONFIG`, offsets below 256).
  - **A legacy line goes to the 8259 PIC,** where the kernel masks every device line but 0, 1 and 2 (timer, keyboard, cascade; `arch/x86_64/interrupts.rs`).
    - A line is unmasked only while a task holds `Capability::Interrupt` for it, and masked again when none does (scheduler.rs).
    - No IOAPIC is programmed; report.rs only counts them.
    - The PIC is set edge-triggered, so a level INTx held asserted gives one edge. On a line held by another driver, that would hide that driver's later edges.
  - **So the bit can be set with no interrupt reaching anyone**, provided that MSI and MSI-X are off and the line is not 0 to 2 (or INTx is disabled, or there is no pin), and no other task holds that line. The HISR0 read clears the halt bits at each check, so an INTx assertion would last until the next check only.
- **What the logs said before this change:** nothing. No boot log carried this device's Command, Status, interrupt line or MSI control.
- **What the Ryzen logs with it** (boots 69 and 70, releases 202611220523 and after, with the constant false):

  `PCI INTERRUPT STATE: COMMAND 0007 (INTX ENABLED), STATUS 0010 (INTERRUPT 0), LINE 255, PIN 1, MSI CONTROL Some(0080), MSI-X CONTROL None`

  - MSI is present but not enabled (64-bit capable, Enable 0), and there is no MSI-X.
  - INTx is enabled on INTA, and the firmware left the interrupt line at 255: none assigned.
  - So `halt_interrupt_refusal` allows the bit on this machine, and by the code above an INTx assertion has no line to reach.
  - That the chipset routes INTA nowhere is not shown by this. The `PCI INTX` field of the HISR0 lines would show the device asserting it.
- **The change:**
  - **The PCI state is logged after the download:** `[RTW89] PCI INTERRUPT STATE: COMMAND c (INTX ENABLED|DISABLED), STATUS s (INTERRUPT b), LINE l, PIN p, MSI CONTROL …, MSI-X CONTROL … (213-DRV-0091)`, read through the capability list (`mac::pci_interrupt`).
  - **`mac::HALT_INTERRUPT_AS_LINUX`:** false, the strict path, keeps HIMR0 masked as now. True leaves B_AX_HALT_C2H_INT_EN on after the download, as `rtw89_pci_enable_intr` does, unless `mac::halt_interrupt_refusal` finds MSI enabled, MSI-X enabled, or INTx enabled to line 0, 1 or 2. In that case it logs why it stays masked.
  - **The guards** (post_mac's three host-mask checks) require every bit clear on the strict path, and all but HIMR0's halt bit when it is left on (`mac::host_mask_required`).
  - **Each `HISR0 HALT BITS` line now gives PCI Status bit 3, `PCI INTX ASSERTED|NOT ASSERTED|UNREAD`,** read before the bits are cleared (`DeviceIo::pci_status`). With the halt bit on, it shows whether the device raised INTx.
- **Host cases:**
  - `the_pci_interrupt_state_says_whether_the_halt_interrupt_may_be_left_on` (tests/rtw89_mac_host.rs) covers:
    - the capability walk (PM, MSI, PCIe, MSI-X);
    - each refusal and each allowance;
    - an unreadable device;
    - the guards' required bits on both paths;
    - `halt_interrupt_on` written and checked.
  - The HISR0 case now expects INTx not asserted under the mask.
  - The 40 `rtw89_*_host.rs` files pass, and the release build builds.
  - With the constant set true, the release build builds and only the case's "the strict path is the default" assertion fails.

**2026-11-22, the mended check on the Ryzen: its first meeting** (boot 74, release 202611220644, from S/rz37 ff2eee6e2. Its rtw89_wifi is fab92488e: the mended check, the firmware's error codes, the HISR0 read, the PCI interrupt state line, `HALT_INTERRUPT_AS_LINUX` false. It was the boot image's driver in boots 69 to 76 from 05:29 UTC):
- **The line, at about 07:19 UTC:**

  ```
  1776.947 rtw89_wifi: [RTW89] CHECK MET AN UNREADABLE CMAC 1: 40018 READ DEADBEEF, READABLE AGAIN AFTER 2 READS IN 1341 US; SER_DBG_INFO 00000000 -> F000001F (213-DRV-0091)
  ```

  - **It met the night's register.** The window, 0x40018, read DEADBEEF as at the night's stops, and the second read 1.3 ms later answered.
  - **The data mode went on.** The minute lines from 1806 to 2286 s show traffic, beacons and acknowledged sends, with no hole and the inbox never full.
- **The firmware recovered during it, silently.**
  - SER_DBG_INFO in the probes read 0 at 66 s, F000001E at 666 and 1266 s, and F000001F at 1866 s, so the meeting came with one step of the count.
  - The BB and the scheduler were never off, so the line says `AN UNREADABLE CMAC`.
  - HISR0 showed no halt bit, and no error code came.
- **A wording to mend later:** the line's "before" value is the session's last-seen SER_DBG_INFO, 0 since the admission with no recovery outlasted, not a read just before the meeting. The step was 1E to 1F.
- **Not yet shown:**
  - This is one meeting only.
  - The count's rise of 30 between 66 and 666 s of this boot, during the display's stage runs, is not explained. In boot 65 it rose by 1 or 2 at a time.

**2026-11-22, the recoveries under load, and an instrument to place them** (reading of boots 74 to 79, then branch `claude/213-DRV-0091-the-recoveries-placed`, on step 2's 3c6a1de2b; host cases, not run):
- **The count's rises against what ran** (each ten-minute window between GOOD LINK probes, `wf-bench/windows.py`; the loads from the bench log):
  - Without boot 74's first ten minutes (+30, during the display's stages 1 to 15 and the start of a duplex audio run), the windows with USB or audio load gave +14 in about 60 minutes. The idle windows gave +5 in about 140, about 2 an hour, as boot 65's night did.
  - The load is not the whole story. A window with the audio run on gave 0, and an idle one gave +2.
- **What the minute lines say about our own task's starvation: nothing either way.**
  - In every window, including those with rises, the driver's minute lines kept their 60 s within 2 to 20 ms, the inbox was never full, and the frames kept stayed at 1000 to 1200 a minute.
  - Sampled once a minute, that cannot show a stall of tens of ms between two polls of the ring.
- **The change:**
  - **Every step of SER_DBG_INFO placed.** Each data-mode check reads SER_DBG_INFO, one register. When it moves:

    ```
    [RTW89] SER_DBG_INFO STEP n: a -> b, SEEN t US AFTER THE CHECK BEFORE; THIS MINUTE'S RX POLLS SO FAR: RXQ LONGEST GAP g US, MOST WAITING w; RPQ … (213-DRV-0091)
    ```

    The step lies between the two checks.
  - **The ring's polls.** `rx::Rings::receive` keeps, for RXQ and RPQ:
    - the longest time between two of its polls;
    - the most descriptors waiting at one poll, the hardware index ahead of the host's, out of 256.

    The minute line gives them and starts them again:

    ```
    [RTW89] TRAFFIC ON c: RX POLLS: RXQ LONGEST GAP g US, MOST WAITING w; RPQ LONGEST GAP g US, MOST WAITING w; OF 256 (213-DRV-0091)
    ```

- **What it tells.** If our task is starved, a step should follow a long gap or a ring near full. If not, steps come with short gaps and a low backlog. The runs planned are an hour under load (duplex audio, with two-camera video if the console allows) and an idle hour, on the same build.
- **Host case:** `service_places_each_step_of_ser_dbg_info_and_counts_the_rings_polls`. It covers:
  - three frames waiting at one poll;
  - a silent recovery's step from 0 to 1, placed with its time since the check before and the polls so far;
  - the minute line's polls, and the next minute's starting again.

  The 40 `rtw89_*_host.rs` files pass, and the release build builds.

**2026-11-22, boot 81: the instrument's first idle half hour** (release 202611221107, S/rz37 57b609354. It is step 2 plus 8e57f1c56. Its USB side changed with it too: CAM's usb_host steps and usb_serial's pieces. Read through the server):
- **Two USB events, neither ours:** the speakerphone left the hub (`XHCI 1: DEVICE ON PORT 1.4 GONE`, 321.1 s, 11:16:42 UTC), and the cable's converter on the PC's side failed (`[LINK] MARK: Unconnected`, 706.5 s, 11:23:07 UTC). From then on there was no load, and boot 81 is an idle run.
- **A meeting with no recovery:** `833.232 … CHECK MET AN UNREADABLE CMAC 1: 40018 READ DEADBEEF, READABLE AGAIN AFTER 2 READS IN 1327 US; SER_DBG_INFO 00000000 -> 00000000`.
  - SER_DBG_INFO did not move at this check or any other, and the boot has no `SER_DBG_INFO STEP` line.
  - The window can read DEADBEEF for about 1.3 ms without a recovery being counted.
  - It came two minutes after the link's mark went Unconnected (11:25:14 against 11:23:07 UTC). The times show no more than that.
- **The ring's polls, 31 minutes to 1866 s:**
  - RXQ's longest gap a minute had a median of 18.8 ms (18.3 to 19.8 ms), and 27.0 ms in the first minute.
  - The most descriptors waiting at one poll had a median of 31, at most 39 of 256.
  - RPQ never had more than 1 waiting.
  - The minutes with a probe (every tenth) were no longer than the others, so the probe does not set the 19 ms.
- **What the design intends and what it does not explain:**
  - Each service step waits at most POLL_MS (2 ms) for a request (`recv_timeout`), then drains up to DRAIN (64) descriptors of each ring. The checks and the coexistence reconcile run once in CHECK_MS (10 ms) after the queue reads empty. So the design's cadence is about 2 ms plus a step's own work.
  - The worst cases written into the loop:
    - a recovery outlasted: SER_WAIT_US 60 ms plus SER_GRACE_US 20 ms;
    - an unreadable window read again: 16 reads 1 ms apart;
    - an H2C waiting its whole COMMAND_US: 100 ms;
    - the service's 16-frame buffer full, which leaves RXQ unread until wlan reads.
  - None of these recurs every minute at idle. So the steady 19 ms is not explained by the design, and its cause is not known.
  - Headroom: at idle about 525 descriptors arrive a second (frames and PPDU reports), so 256 last about half a second. Even 100 ms leaves half the ring.
- **To place the 19 ms** (this commit): the minute line also gives each ring's gaps over 5 ms and over 10 ms, and how long ago the longest ended:

  ```
  [RTW89] TRAFFIC ON c: RX POLL GAPS OVER 5 / 10 MS: RXQ a / b, RPQ c / d; THE LONGEST ENDED x / y MS AGO (213-DRV-0091)
  ```

  - Many such gaps a minute would be periodic work of our own.
  - A handful at random offsets would be the scheduler.
  - A fixed offset from the minute line would be the minute's own work.

  The new host case is `the_rings_keep_their_poll_gaps_and_backlog_until_taken` (tests/rtw89_rx_host.rs). The 40 `rtw89_*_host.rs` files pass, and the release build builds.

**2026-11-22, boot 82: sends held in the MAC; what holds them, and the calibrations' results** (branch `claude/213-DRV-0091-the-recoveries-placed`; host cases, not run):
- **What boot 82 showed** (release 202611221215, read from the stick; boot 83 is the same image and sent well):
  - 22% of sends late: 228 of 1027 over 8 minutes, against at most 2.3% in the 33 other driver starts of boots 55 to 83 with minute lines.
  - Of its 221 late frames, 155 were in the end acknowledged at the first try. They were released at a median of 222 ms after their submission (90th percentile 832 ms, longest 2.5 s), after the TX DMA had fetched them.
  - So the MAC held the frames and then sent them once. The access point did not fail to acknowledge them.
  - Every other boot shows the same kind of late frame at a low rate (boots 65, 76, 79, 81: all at least half acknowledged at the first try, medians 138 to 199 ms).
  - The starts, line by line, and the register dumps at the port step and in the first minute (their pieces joined) do not tell boot 82 from 80, 81 and 83.
  - Its first minute had RXQ MOST WAITING 255 of 256, where boots 81 and 83 had 34.
- **The change:**
  - **A late frame probed while the MAC holds it.** It reads directly, with no RF read, no indirect window and no H2C: `data_mode::HELD_MAC` and `HELD_BB`. These are the coexistence control byte and scoreboard, MACID pause and sleep, CTN_TXEN, port 0's PORT_CFG, TBTT_PROHIB, BCN_AREA, the early-interrupt times, BCN_SPACE, BCN_CNT_TMR and the TSF, the PTCL settings, Linux's coexistence monitor set, the RX ring indexes, and BB's 0x980, the EDCCA level and its reports, and TX collision.
    - The first 8 late frames of a run are probed, then at most one a minute.
    - The line gives the time since the frame's submission, and where in the beacon interval it was submitted and is now: by our clock since the last beacon taken from the ring, and by the TSF modulo the beacon's own interval.
    - One frame released in time after the first held one is probed once beside them:

    ```
    [RTW89] HELD FRAME (LATE n): t MS SINCE ITS SUBMISSION; SUBMITTED a / b MS INTO THE BEACON INTERVAL OF i MS (OUR CLOCK / TSF), NOW c / d (213-DRV-0091)
    [RTW89] HELD FRAME'S REGISTERS: … (213-DRV-0091)
    [RTW89] A FRAME RELEASED IN TIME, BESIDE THE HELD ONES: …
    ```

  - **The minute line:**

    ```
    [RTW89] TRAFFIC ON c: LATE FRAMES RELEASED n: m ACKNOWLEDGED AT THE FIRST TRY, k NEVER; THEIR MEDIAN HOLD h MS; THE FRAME BUFFER FULL f TIMES, THE LONGEST l MS (213-DRV-0091)
    [RTW89] TRAFFIC ON c: RELEASES BY THE BEACON INTERVAL'S PHASE, 10 MS BINS: BY OUR CLOCK [...]; BY THE TSF [...] (213-DRV-0091)
    ```

    A hold locked to the TBTT would pile the releases into a few bins. By our clock the phase is good to the ring's poll cadence. By the TSF it holds only while the port's TSF follows the access point's, which is step 2's TSF_UDT.
  - **The calibrations at admission,** two lines a path, each with PASS or FAIL by Linux's criteria in rtw8852b_rfk.c at 05f7e89:
    - IQK: LOK's IDAC and VBUF words in range, as `_lok_finetune_check` (line 1147; the ranges at 1163 and 1174: core 0x2 to 0x1D, VBUF 0x2 to 0x3D). The one-shots' failures, NCTL's report flag read by `_iqk_check_cal` (line 253) for each `_iqk_one_shot` (line 815). The restore.
    - DPK: TXAGC, thermal, and the sync's correlation and DC against `_dpk_sync_check` (line 1974; DPK_SYNC_TH_DC_I/Q 200 and DPK_SYNC_TH_CORR 170 at 1976 to 1978, the test at 2009). This driver's own sync already refuses beyond them (dpk.rs), so a DPK FAIL cannot reach the data mode.
    - TSSI: the alignment words and the two reports. Linux sets no threshold for them, only that the alignment was done (`_tssi_alimentk_done`, line 3374).
    - DACK runs once at the initial PHY and its result is not kept, so it is not printed. Linux's criterion is its done-polls timing out (lines 601 to 638).
  - **A bench switch, `service::PORT_AS_LINUX`:** true, as now, sets port 0 for INFRA once linked (step 2). False leaves the port as before step 2, for a series of starts without it.
- **Host cases:**
  - `service_probes_held_frames_and_counts_how_the_minutes_sends_ended`. The model holds sends' final reports back. Ten late frames get eight probes and one more in the minute, an in-time frame gets its probe once, and the minute line counts 10 late, 10 at the first try, a median hold over 90 ms and the frame buffer's full spell.
  - `calibration_results_are_judged_by_linuxs_criteria`: the DPK and IQK criteria at their edges.
  - The 40 `rtw89_*_host.rs` files pass, and the release build builds.
- **What a late answer does upstream.** From the code at 2e854da9c; `wlan` and `dot11` are main's at 1e9d8beb5. Nothing is changed. True of every boot at its own low rate of late frames:
  1. **One send at a time.** A send while one is pending is answered Busy at once (rtw89_wifi/src/service.rs:536).
  2. **The deadline.** A send's deadline is COMMAND_US less FRAME_MARGIN_MS, 100 − 10 = 90 ms (service.rs:537; the constants at 17 and 20).
  3. **The late answer.** At the deadline, KEEP_LATE (service.rs:29) answers `Sent { acked: false, attempts }` with the tries seen so far (service.rs:265). It keeps the frame pending, its release awaited LATE_MS (5000 ms, service.rs:30) more (service.rs:262). Every send meanwhile is refused as in step 1.
  4. **wlan waits for the answer.** `wlan`'s send is a synchronous call under its deadline (wlan/src/main.rs:468, `radio_call` at :60), so wlan is blocked for the 90 ms.
  5. **wlan records the outcome.** A unicast data frame's answer goes to `station.sent(sent.acked, sent.attempts)` (wlan/src/main.rs:469). Any error, Busy included, goes to `station.sent(false, ATTEMPTS)` with ATTEMPTS 15 (wlan/src/main.rs:470 and :40). The frame is not sent again: a held frame does go out later, and a refused one is dropped.
  6. **The rate steps down.** `dot11::Station::sent` (dot11/src/station.rs:443) steps the rate down at once for a frame not acknowledged (station.rs:449), and its tries count against that rate's success, which chooses the rate every RATE_WINDOW frames.
  - **So one hold causes:**
    - a false loss and a step down in rate;
    - every frame sent behind it refused and lost, each another step down with 15 tries counted;
    - wlan blocked for 90 ms.

    TCP loses the refused frames. The held one reaches the peer once, unless TCP's timer runs out first. At boot 82's rate of holds, that is why the uploads failed outright.
  - **What I would try, once the series has shown what holds the frames:**
    - (1) answer a late send at its release, up to a bound (about 1 s), with its true tries and outcome;
    - (2) queue sends behind a held one instead of refusing them.

    (2) changes wlanhw's contract and wlan's handling of Busy, so it is NET's as well.
- **The frame buffer:** until now nothing logged its full spells. The minute line now counts them and gives the longest. With the ring's polls every 20 ms or less, boot 82's 255 waiting needs RXQ left unread for about 0.4 s, which is the buffer full while wlan did not read. A wlan blocked in its synchronous sends is one way that happens.

**2026-11-22, the service's steps timed** (branch `claude/213-DRV-0102-floor-and-answer`; host cases, not run):
- **Why:** the RX ring's poll gaps over 5 ms (300 to 450 a minute, the longest 18 to 21 ms) still have no cause. The owner's `ps` on boot 97 shows `rtw89_wifi` on CPU 4 with `compositor` and `clipboard`, on a machine of 16 CPUs.
- **A minute line:**

  ```
  [RTW89] TRAFFIC ON c: SERVICE STEPS: WAITS OVER 5 / 10 MS a / b, THE LONGEST w US; WORK OVER 5 / 10 MS c / d, THE LONGEST k US (213-DRV-0091)
  ```

  - A step's wait is its `recv_timeout` of POLL_MS (2 ms; IPC timeouts end at the first 1 ms tick after the time asked on x86, docs/api).
  - A step's work is the rest of the step: the rings' polls, the checks, the dispatch.
  - A wait far over 2 ms without a request is the task not run again on time: the scheduler, or another task on its CPU, which is KRN's. Work over it is the step's own.
- **Host case:** the minute sends case checks both at zero over 5 ms in the model, whose receive wait ends at once.

**2026-11-22, boot 99: the poll gaps are late wake-ups of an idle machine** (release with ca9da3f25; cable reads at 18:23 and 18:28 UTC):
- **The SERVICE STEPS line:** in every minute without the cameras, the 2 ms receive wait came back after more than 5 ms 373 to 431 times, and after more than 10 ms 252 to 311 times, the longest 18 to 21 ms. The step's own work never passed 5 ms, apart from the tight poll's spin. The RX poll gaps are those waits, one for one.
- **With CAM's two cameras streaming** (465 to 650 s): 0 waits over 5 ms in two whole minutes (552 and 612 s), the longest 2,979 and 3,204 µs. The minutes at the edges had 194 and 144. Boot 98's camera minute had shown the same: 36 against 294 to 463.
- **What the kernel's code says** (read, not changed):
  - IPC deadlines end only on the boot CPU's 1 ms tick (`Scheduler::timers` when cpu == 0, kernel/src/scheduler.rs:1994, 1997, 2070, 2071; :369 to 371; TICK_MS, kernel/src/arch/x86_64/interrupts.rs:7).
  - `expire` (scheduler.rs:571) readies the task, and `wake_idle` (scheduler.rs:317 to 333) sends an idle CPU one wake IPI until it selects again.
  - An idle CPU halts in `scheduler::idle` (scheduler.rs:2095 to 2100; `cpu::wait_for_interrupt`, kernel/src/arch/x86_64/cpu.rs:394), with no tick of its own (scheduler.rs:325).
- **So the late wake-ups come from the kernel's idle-CPU wake path:** a late tick or timers pass on CPU 0, or a wake IPI not sent while `woken[]` stays set. They do not come from the driver or from CPU 4's sharing. That is KRN's, and the owner carries it there.
- **Not shown:** which of those paths it is. A kernel counter per CPU would tell: IPIs sent, IPIs suppressed, timers passes late.
- **Under uploads** (the same boot, eleven minutes at 853 to 1513 s, 400 sends a minute): 301 to 431 waits over 5 ms a minute, as on the quiet machine. Only the cameras' whole minutes had none. The figures are in [213-DRV-0104](213-DRV-0104-the-sends-answer-the-drivers-side.md), boot 99's figures.

## For a successor (state on 2026-11-22)

**Branches and heads** (the WIFI agent's; worktrees under the session's scratchpad, `a-*`):

| Branch | Head | State |
|---|---|---|
| `claude/213-DRV-0087-rtl8852be-on-5-ghz` | ef2bb4f65 | In main (round 0748) |
| `claude/213-DRV-0090-the-receiver-goes-deaf` | 96784295b | In main (round 0748) |
| `claude/213-DRV-0091-the-firmware-told-it-is-connected` (step 1) | e4fbbd891 | In main (round 0748) |
| `claude/213-DRV-0091-the-halt-interrupt` (the mended check, the error codes, HISR0, the PCI state, `HALT_INTERRUPT_AS_LINUX` false) | 5b105801a | In main (round 170). Superseded heads: c6202f1c8, 07e59deb9, 551b174ce |
| `claude/213-DRV-0091-step-2-the-port` | 3c6a1de2b | Main 1e9d8beb5 merged in, so the port alone over main. Full suite green (261, 0 failed). In the boot image since boot 80. Waits for the starts series and a night that shows it does no harm |
| `claude/213-DRV-0091-the-recoveries-placed` (on step 2) | 2e854da9c | The SER steps placed, the ring's polls and gaps, the held-frame probe, the minute's sends, the calibrations, `PORT_AS_LINUX`. In the bench tree (2fe8f4319, the release of 13:35 UTC). b7ade4b60 green (261, 0 failed); 2e854da9c's full run was in progress at this writing. Waits for the loaded hour and the series |
| `claude/213-DRV-0091-halt-interrupt-on` (bench only, never to land) | a99e72427 | Flips `HALT_INTERRUPT_AS_LINUX` to true on 3c6a1de2b. Not run. The next one-change run after step 2's verdict |
| `claude/550-DRV-0088-the-owners-rule` | 068c5e589 | Green 254 then. Waits for NET (wlanhw) |

**Bench switches:**
- **`mac::HALT_INTERRUPT_AS_LINUX`** (false): never run true. a99e72427 is the flip. On the Ryzen, `halt_interrupt_refusal` allows it (INTx on INTA, line 255, MSI off).
- **`service::PORT_AS_LINUX`** (true): step 2's port. Every boot from 80 ran it true, and the starts series runs it true. False is the series to run if the held probes show the port's TBTT prohibit holding the frames.
- The driver's older DEBUG constants (KEEP_LATE, OUTLAST_SER, OWN_DEADLINES) are as they were.

**The collector and the tools** (wf-bench/ in the session's scratchpad):
- **`collect-day.sh FIRST_BOOT READS`:**
  - Every 30 minutes it fetches the current boot's file through the server (`hes-op.sh … collect_file log:/bootNNNN.log`, `max_bytes` 1500000) into handoff/wifi/day/.
  - It skips a read while anyone but WIFI holds the machine.
  - It exits at the first new CHECK MET, SER_DBG_INFO STEP, HISR0, error code, recovery, stop or hole line (seen.txt holds those already reported).
- **Analysis scripts:**
  - `day_report.py`: a boot's drivers, SER_DBG_INFO, meetings and line kinds;
  - `windows.py`: the ten-minute windows against the count's rises;
  - `traffic_minutes.py`: beacons and late sends a minute;
  - `ser_per_driver.py`;
  - `probe_values.py`: a probe's pieces joined, the values that differ between boots;
  - `starts_table.py`: one row a start for the series. The series' logs come into handoff/wifi/starts/.
- **The rtw89 host files:**
  - `wf-bench/hosttests.sh`, its path and output folder changed with sed for each worktree, builds each `tests/rtw89_*_host.rs` with `rustc --edition=2021 --test`, the service file with `--cfg rtw89_service`, and runs it.
  - The release build is `cd rtw89_wifi && CARGO_TARGET_DIR=… cargo build --release`.
  - The full suite runs under `flock /tmp/claude-1000/heavy.lock` with `MIND_SSHD=/tmp/claude-1000/sshd-pkg/root/usr/sbin/sshd bash scripts/host_tests.sh`. A fresh worktree runs `./02_build.sh` first.
  - APP's `improve` case failed now and then until its fix in round 170.

**What is shown:**
- **The mended check works on the Ryzen:** CHECK MET lines in boots 74, 76 and 81, with no stop. The 2 ms wait of c6202f1c8 was refused by `io::short_delay` and stopped the data mode five times in boots 64 and 65.
- **The firmware's recoveries are silent:**
  - no HALT_C2H code and no HISR0 halt bit in boots 65 to 83;
  - SER_DBG_INFO counts them;
  - about 2 an hour idle, about 14 an hour in the windows of USB or audio load (boots 74 to 79).
- **The error masks match Linux's.**
- **The PCI interrupt state:** INTx on INTA, line 255, MSI present and off.
- **Step 2 sets the port** (C400 1E81C, INFRA). No rate difference is shown.
- **Late frames are held in the MAC, then sent once and acknowledged:** boot 82 at 22%, every other boot at 2.3% or less. That makes one bad start in 34 of boots 55 to 83.

**What is open:**
- **What holds the frames.** The series and the held probe will tell, TBTT prohibit first.
- **The poll cadence:** 18 to 21 ms longest gaps and about 400 gaps over 5 ms a minute. The same in a good boot and a bad one, so not the cause of the late sends. It is not mended before the loaded hour shows whether RXQ's backlog grows.
- **Whether a recovery follows our own task's starvation.** The loaded hour waits for the cable and the console.
- **Unexplained readings:**
  - boot 81's meeting with no SER step;
  - boot 74's rise of 30 in its first ten minutes;
  - boot 82's RXQ backlog of 255 in its first minute.
- **5 GHz** (213-DRV-0087): the first join waits for NET's first-join wait and for 550-DRV-0088 (the owner's rule), which waits for NET's wlanhw.
- **The answer path for late sends** (above) is NET's contract question, carried by the owner after the series.

**Traps:**
- **A probe's value can be cut in two.** A probe is printed in pieces of a fixed width, and a register's value may straddle two pieces. Join the pieces' text exactly before comparing (`probe_values.py`); read piece by piece, 0x4688 and 0xC70 once looked different.
- **The minute's own printing lands in the next minute.** A minute's polls are taken when its TRAFFIC event is built, before its lines are printed. So the printing falls into the next minute, and a longest gap caused by it would show "ago" of about 60 s.
- **CHECK MET's "before" is stale.** Its SER_DBG_INFO "before" is the session's last-seen value, not a read just before. The SER_DBG_INFO STEP line has the step.
- **`io::short_delay` refuses more than 1 ms.** The host model now refuses it too. The model's bus clock is not the IPC clock, so gaps and phases in host cases are the model's.
- **A server read during another agent's usb_host replace latches the Effector agent** (its journal on the stick) until the machine restarts. Read outside others' leases.
- **A boot that sends badly cannot upload its log** (collect_file `upload`, boot 82). The log is then read from the stick under Linux.
- **The late frame's `ANSWERED NOT ACKNOWLEDGED` is not the access point's.** It is our 90 ms answer. The LATE FRAME WAS RELEASED line that follows has the outcome.

## Related

[213-DRV-0090](213-DRV-0090-the-receiver-goes-deaf-on-a-busy-channel.md) (the holes, the beacons not on the host; this branch's base), [213-DRV-0032](../issues-done/213-DRV-0032-rtl8852be-wifi.done) (the driver), [213-DRV-0052](213-DRV-0052-coexistence-after-association.md) (coexistence after association).
