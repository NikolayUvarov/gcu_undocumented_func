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

## Related

[213-DRV-0090](213-DRV-0090-the-receiver-goes-deaf-on-a-busy-channel.md) (the holes, the beacons not on the host; this branch's base), [213-DRV-0032](213-DRV-0032-rtl8852be-wifi.md) (the driver), [213-DRV-0052](213-DRV-0052-coexistence-after-association.md) (coexistence after association).
