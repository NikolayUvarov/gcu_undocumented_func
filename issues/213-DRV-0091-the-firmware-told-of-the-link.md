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

**2026-11-22, why the driver masks HIMR0's halt bit, and HISR0 read under the mask** (branch `claude/213-DRV-0091-the-halt-status`, on 07e59deb9; host cases, not yet run on the Ryzen):
- **Who masks the bit, and why.** `mac::mask_host` came with 71589b8b9 (213-DRV-0032, 2026-11-04, "the bounded diagnostic passes on the Ryzen").
  - Its message: "the three host interrupt masks cleared, before the MAC preparation and after the download (the firmware enables HIMR0's halt interrupt), as disable_intr does".
  - main.rs's note: "The firmware enables its halt interrupt (HIMR0 bit 21); this polled driver keeps host interrupts masked".
  - 213-DRV-0032's table records it: HIMR0 `00200000` after the download, 0 before.
  - So the reason is that this driver polls and has no interrupt handler. Masking was to follow `rtw89_pci_disable_intr`, the state Linux keeps before it starts. Nothing in the issue or the code says the bit was masked to change the firmware's behaviour.
- **What Linux does with the bit** (pci.c, pci.h and rtw8852be.c at 05f7e89):
  - The 8852BE uses `rtw89_pci_config_intr_mask`, which sets `halt_c2h_intrs = B_AX_HALT_C2H_INT_EN` (bit 21; WDT_TIMEOUT, bit 22, is only in the recovery masks of the v1 chips).
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

## Related

[213-DRV-0090](213-DRV-0090-the-receiver-goes-deaf-on-a-busy-channel.md) (the holes, the beacons not on the host; this branch's base), [213-DRV-0032](213-DRV-0032-rtl8852be-wifi.md) (the driver), [213-DRV-0052](213-DRV-0052-coexistence-after-association.md) (coexistence after association).
