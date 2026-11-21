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

## Related

[213-DRV-0090](213-DRV-0090-the-receiver-goes-deaf-on-a-busy-channel.md) (the holes, the beacons not on the host; this branch's base), [213-DRV-0032](213-DRV-0032-rtl8852be-wifi.md) (the driver), [213-DRV-0052](213-DRV-0052-coexistence-after-association.md) (coexistence after association).
