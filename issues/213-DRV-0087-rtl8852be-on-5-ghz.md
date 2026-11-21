# 213-DRV-0087 — The RTL8852BE on 5 GHz: 20 MHz channels at 802.11a's rates

**Type:** driver (`rtw89_wifi`, its tables and their generators) · **Owner:** `DRV` · **Priority:** P1 · **Status:** open (2026-11-21) · **Blocked by:** the branch of [213-DRV-0032](213-DRV-0032-rtl8852be-wifi.md)'s change of 2026-11-21 in `main` (the base); NET's word on `wlan` · **Main task:** [213](213-amd-ryzen-pc.md) · **Constitution:** MC-6.1, MC-12.1, MC-12.3

## Problem

The driver was brought up on the narrowest configuration, as [213-DRV-0032](213-DRV-0032-rtl8852be-wifi.md)'s plan said: 2.4 GHz, 20 MHz. Every table and calibration in it is that band's.

On the Ryzen's network, the same machine and access point, 2026-11-21:

| | 5 GHz, channel 100 | 2.4 GHz, channel 8 |
|---|---|---|
| Linux | 16.4 MB/s (VHT 80 MHz) | 480 to 826 KB/s (HT 20 MHz) |
| MIND Core | not there | 252 to 353 KB/s |

The 2.4 GHz channel is crowded, and its antenna is shared with Bluetooth. The 5 GHz one is neither.

**What is there already:** `wlanhw.wit` 2.0 has the 5 GHz bit in `bands` and room for its channels, and `dot11`'s country and station code name the 5 GHz channels and their rates. Whether `wlan` scans and joins there is asked of NET.

## Scope

- **In:** the 20 MHz channels of 5 GHz the country allows, at 6 to 54 Mb/s.
- **Not in, each a task of its own later:** HT and VHT, wider channels, aggregation. So this task will not reach Linux's 16 MB/s; what it does reach is to be measured.
- **Channels with radar rules** (52 to 144; the Ryzen's network is on 100): the station receives there and sends only after it has heard the access point's beacon on that channel. No probe request before that. The driver's calibration transmits too, so it follows the same rule: a channel is calibrated when the station first sends there, as now.

## Plan

1. **Sources and tables,** from the pinned Linux sources as for 2.4 GHz, each recorded in `THIRD_PARTY.md` and made by a generator in `scripts/hardware/` that reproduces it:
   - the baseband's and the radio's channel setting for 5 GHz;
   - the receive gain tables and the board's gain offsets for its sub-bands;
   - the transmit power by rate and the limits by regulatory domain;
   - TSSI, IQK and DPK for 5 GHz.
2. **The channel policy and the country catalogue:** the 5 GHz channels with their limits, listen-only where the rules say so; `info`'s `bands` and `supported`.
3. **The band switch:** what Linux does when the band changes, the coexistence's side included (on 5 GHz the station does not share Bluetooth's path).
4. **A reference from Linux on channel 100,** as was made for channel 8 on 2026-11-21: the registers of the MAC, the baseband, the radio and the calibration with a link up, compared with what this driver leaves.
5. **On the Ryzen, a step at a time:**
   - a receive-only listen on channel 100 hears the access point's beacons;
   - the calibration there completes;
   - the station joins the 5 GHz network;
   - 6 MB from the LAN, measured as on 2026-11-21 (the sender's `TCP_INFO`), beside the 2.4 GHz figure of the same hour.
6. **2.4 GHz stays as it is:** its host cases pass unchanged, and the Ryzen still joins on channel 8.

## Acceptance criteria

- [ ] Host cases cover the 5 GHz channel setting, the policy and its limits, and that nothing is sent on a radar channel before a beacon is heard there. The generators reproduce the tables.
- [ ] On the Ryzen the scan lists the network on channel 100, the station joins it, and 6 MB arrive at a recorded rate. This holds for that machine, that access point and that channel (MC-12.1).
- [ ] The Ryzen still joins on channel 8 with the same build, and its rate there is recorded.
- [ ] The issue says what was not done: other channel widths, HT and VHT, other machines.

## The data model (2026-11-21)

- **The band is the channel's.** A channel number names its band and centre: 1 to 13 are 2.4 GHz (2407 + 5n MHz); the 28 channels of Linux's `rtw89_channels_5ghz`, 36 to 177, are 5 GHz (5000 + 5n MHz). No separate band field travels beside a channel. `rtw89_wifi/src/band.rs` holds the list, `band`, `subband`, `center_mhz` and `channel_at`.
- **Sub-bands** are Linux's `rtw89_subband` for this chip: 2.4 GHz; 5 GHz low (36-64), middle (100-144), high (149-177). Each gives the board's gain-compensation index, its OFDM gain-offset index and the BB gain table's band.
- **The initial PHY stays on 2.4 GHz.** The DM prefix, DACK, RX DCK and the path tail run once at start on channel 1, as today. A 5 GHz channel is set from there by the channel stage, as Linux's `set_channel` does after its own 2.4 GHz start. So `prefix.band` is the start's band; a tuned channel's band is its own.
- **The country's 5 GHz rules** come from the radio's own copy of the catalogue: the one rule that holds the whole 20 MHz channel. DFS and NO-IR make a channel listen-only. A WMM rule (the driver does not set the channel-access limits it asks for), NO-INDOOR or NO-OUTDOOR (the environment is unknown), NO-OFDM and point-to-point rules leave it out. Each country's 5 GHz power group is `COUNTRY_REGD`'s second column.
- **A listen-only channel** (radar or NO-IR): the receive-only service keeps the access points whose beacon or probe response it received there since the tune. A `send` there admits the channel only if it is not a probe request and goes to one of them; that evidence travels with the admission, so the calibration's own bursts too come only after a beacon was heard. In data mode a send there goes to that access point alone, while its beacon was heard within the last 5 s (half the station's own silence of 10 s, after which the service stops anyway), never a probe request.
- **5 GHz is OFDM alone:** `info.bands` is 7 (2.4 GHz, its OFDM, 5 GHz); the data service refuses a rate under 12 (6 Mb/s) on 5 GHz with `invalid`.

## What the catalogue allows on the bench (2026-11-21)

The station takes its country from the access points when none is chosen: on the Ryzen that is **RU** (announced by 4 of the 8 that named one, the log of 2026-11-21). Read from the pinned catalogue, and the driver's host cases agree:

| Country | 5 GHz under the driver's and the station's rules |
|---|---|
| RU | none: every 5 GHz rule is NO-OUTDOOR, and channel 100 is outside them |
| UZ | 36-48 may be sent on (20 dBm); 52-64 listen-only (DFS); 100 is outside the rules (they end at 5330 MHz) |
| 00 | 36-165 listen-only (NO-IR or DFS) |
| US | 36-48 and 149-165 sendable; 52-144 listen-only (DFS) |

So on this bench the station never joins channel 100 lawfully: under RU and UZ it is outside the rules. A join on 5 GHz needs a person's decision: the access point's 5 GHz network on a channel the country allows (UZ: 36 to 48), and that country chosen for the station if it is not the one announced. Receiving on channel 100 needs nothing: listening is allowed everywhere.

## Progress

**2026-11-21, the channel setting, the tables and the rules (commit of this section):**
- `band.rs`, as above; `dack::Band` and `phy::tail::Subband` are its types now.
- **The 5 GHz power tables:** `scripts/hardware/rtw89_power_tables.py` extracts `rtw89_8852b_txpwr_lmt_5g` (all four bandwidths), `txpwr_lmt_ru_5g`, the 5 GHz transmit shapes and by-rate rows from the same pinned `rtw8852b_table.c`, `--check` reproduces `power_tables.rs`; `channel_policy.rs` decodes them at Linux's channel slots (`rtw89_channel_to_idx`), CCK's by-rate from the 2.4 GHz table as Linux reads it.
- **The power image** (`channel_power.rs`) of a 5 GHz channel: the 5 GHz tables, no CCK DFIR (50 writes, not 58); the 75 % cap of an unknown country leaves CCK out there.
- **The channel stage** (`phy/channel.rs`) sets a 5 GHz channel as `rtw8852bx_set_channel_bb`, `__rtw8852bx_set_channel_mac`, `rtw8852b_bb_reset_en` and `_ch_setting` do; the table of differences is in [the channel-stage notes](../hwdocs/rtl8852be-channel-stage.md#5-ghz-at-20-mhz-213-drv-0087). A 2.4 GHz tune after a 5 GHz one puts the tail's BTG share back.
- **The services:** `info` lists the 41 channels and bands 7; `set-channel` takes the 5 GHz ones; the passive tune and the listen take their centres; received 5 GHz frames carry their channel.

**Host cases:** the country's 5 GHz rules (UZ, US, 00, RU, DE); the 5 GHz tables at Linux's slots; the 5 GHz power image and its maximum; a passive tune to channel 100 and back to 6 with each band's registers read back; nothing admitted on a radar channel before its access point's beacon, a probe request never, and a tune forgetting what was heard; a receive-only listen on channel 100. All 39 `rtw89_*_host.rs` files pass. Changed by design, not by the 2.4 GHz path: the info cases now see 41 channels and bands 7, the world rules count 38 channels (25 of them 5 GHz), and the listen case's refused frequencies no longer include 5180 MHz.

**2026-11-21, IQK on 5 GHz:** `iqk.rs` takes the band of the tuned channel: `_lok_res_table`'s 5 GHz entry, `_iqk_txk_setting`'s 5 GHz branch (XGLNA2, BIASA, LUTWA 0x80), LOK's GR1 of 4, `_txk_group_sel`'s `_a_` track ranges and ITQT, `_rxk_group_sel`'s `_a_` gains on RR_RXA2's HATT and CC2 with RXK's 5 GHz select, RF18 checked with its band fields, IQKCH's band field, RX DCK and the BTC notices with the band in their phy map. Host case: channel 100 runs both paths' 26 one-shots with each of those values and restores the RF backups; a 2.4 GHz RF18 refuses it before any command. The 2.4 GHz IQK cases pass unchanged.

**2026-11-21, TSSI on 5 GHz:** `scripts/hardware/rtw89_tssi_tables.py` also extracts `rtw8852b_tssi_sys_{a,b}_defs_5g`, `slope_{a,b}_defs_5g`, the six `align_{a,b}_5g{1,2,3}_all_defs` and the three sub-band rows of `_txpwr_track_delta_swingidx_5g{a,b}_{n,p}` from the same pinned files (`--check` reproduces `tssi_tables.rs`). `phy/tssi.rs` takes RR_TXPOW's TXA on 5 GHz, the band's system and slope tables, the sub-band's alignment and thermal tables, the sub-band's gain offset and no BTG in the PMAC samples, leaves CCK's CCA disabled after a PMAC stop on 5 GHz (`rtw8852bx_bb_set_pmac_tx`), and writes the DE from Linux's OFDM, trim and CCK groups (`_tssi_get_ofdm_group` with its averaged extra groups, `_tssi_get_trim_group`); its notices carry the band. Host cases: the groups of every channel against Linux's tables (the 2.4 GHz ones as before), the 5 GHz thermal rows, and the engine on channel 100 with TXA, the 5 GHz system and alignment values, no BTG and CCA kept off. The 2.4 GHz TSSI cases pass unchanged.

**2026-11-21, DPK on 5 GHz:** `dpk.rs` takes `_dpk_rf_setting`'s 5 GHz branch (RR_RXA2's SWATT, RR_RXA_LNA 0x920FC, RR_XALNA2 0x2C0, RR_IQGEN 0x38800, all three in the RF backup Linux restores) and `_dpk_idl_mpa`'s MDPD order 2 for a 5 GHz channel under 80 MHz; its notices carry the band. No external PA is assumed on either band, as before (`_dpk_bypass_check`'s ePA flags are not read). Host case: channel 100 with those RF values, order 2 on both paths, the BB, KIP and RF backups restored; channels 14, 34 and 178 refused before any write. The 2.4 GHz DPK cases pass unchanged.

**2026-11-21, a line for the bench:** the receive-only service logs the first 16 access points it hears on 5 GHz channels in a run (`[RTW89] HEARD <BSSID> ON 5 GHZ CHANNEL n`), from the same beacons its listen-only rule keeps; host case in the radar-channel case.

**2026-11-21, the Linux reference on channel 100** (taken by the owner session: `rtw89` on Linux 7.0.0-43, BSSID `A0:36:BC:11:F8:8C`, VHT 80 MHz centred on 106, beacon −67 dBm, 6 MB in 0.30 s, ETSI power group):
- **Little of it could be read.** Linux had the chip in power save (`btc_info`: `power_save:1`), so its `mac_reg_dump` read `0xEAEAEAEA` for the MAC pages 0x8000 and 0xC000 and most of the baseband, and its `rf_reg_dump` `0xFFFFFFFF`. Readable were TSSI path B's block (0x7600-0x78F0) and the calibration results (0x8000-0x82FF).
- **What agrees:** 0x7600 = `3F2D2721` and 0x7604 = `80010101`, `tssi_align_b_5g2_all_defs`'s values, the table this TSSI takes for 100-144; both paths' IQK enable 0x8124/0x8224 = `505` (TX and RX calibrated); DPK's RF mode in 0x8078 = `50220`; and the coexistence there: link mode 5G, policy OFF_EQ0, antenna W5G, AFH off (`en:0/ch:0/bw:32`).
- **Linux's `txpwr_table` agrees with the 5 GHz decoders** on every width-independent row: by rate (OFDM 20/19/18 dBm, MCS 20 to 13, CCK from the 2.4 GHz table), OFDM_1TX 14, OFDM_2TX 11, MCS_1TX_20M 15 and MCS_2TX_20M 12/9 (beamformed) on 100-112, CCK limits 0. A host case says so.
- **Not checked against Linux,** for want of a readable dump or because Linux ran 80 MHz: every channel-stage register (band select, SCO, gain tables and offsets, CCK off, channel index, RF18, the MAC's rate check), the 20 MHz limits' writes, TSSI path A and the DE, IQK's and DPK's 5 GHz settings at 20 MHz (Linux's DPK ran MDPD order 0, as it should at 80 MHz).

**2026-11-21, on the Ryzen: 5 GHz heard** (`rtw89_wifi` of bf441efca put in with `replace`, release 202611211156, MIND Core on Wi-Fi alone):
- A station whose radio comes back after more than 10 s of silence starts anew with a scan that only listens, on every channel the radio lists. For that, the build was started 15 s late (a bench-only sleep, not committed).
- **The scan:** 40 passive tunes, the longest 4.3 ms, none failed. Heard on 5 GHz, each logged once: `A0:36:BC:11:F8:8C` (CoolNet2) **on channel 100**, and seven others on 36, 44, 48, 52, 60 and 157.
- **Then, as the catalogue says:** the access points announced RU (5 of 13), under which no 5 GHz channel is left; the station joined CoolNet2 on channel 8 with the same build (`DATAPATH ADMITTED ON CHANNEL 8`, `JOINED (TRY 1)`, DHCP).
- **6 MB from the LAN on channel 8,** the sender's figures: this build 159, 242 and 215 KB/s; the release's driver right after it, same minutes, 184, 262 and 277 KB/s. Three runs each on a crowded channel: no difference is shown either way, and none is claimed. The 2.4 GHz data path is the same code in both.
- The signal the driver reports for a frame is 0 (not measured) on both bands: the 8-byte header's byte 2 was never filled by this driver. So NET's "strongest wins" picks by the scan's order here, not by signal.
- **Not done there:** nothing was sent on 5 GHz. Under RU the station does not go there, and channel 100 is outside RU's and UZ's rules (see above).

**2026-11-21, the coexistence on 5 GHz and the admission end to end:**
- **The station connected on 5 GHz** is told as Linux's `_action_wl_5g`: the role (`rtw89_fw_h2c_cxdrv_role_v2` with link mode `BTC_WLINK_5G` and the role's band 1), the antenna W5G (WLAN's grant forced high, Bluetooth's left to hardware, no pollution table: grants `33003300`, 0xC67C = `100`, control by WLAN), the policy OFF_EQ0 (OFF slot `FFFFFFFF`, TDMA off), AFH off; no S1 standby (`_set_rf_trx_para` keeps it off for a 5G link). The coexistence's BTG rule leaves BTG off while the band calibrated or connected on is 5 GHz (`_set_btg_ctrl`), and its AFH never names a 5 GHz channel. The Linux reference shows the same link mode, policy, antenna and AFH there.
- **Two 2.4 GHz assumptions the end-to-end case found:** the calibration's TSSI power check read the 2.4 GHz MCS limit (it takes the band's now), and the data mode's register check wanted the channel number in 0x734 and RF18 without its band fields (it wants `rtw89_encode_chan_idx` and the band fields now). Either would have stopped a 5 GHz admission on the machine.
- **Host cases:** the 5 GHz role and OFF_EQ0 against Linux's layouts; the whole admission on channel 100 in the models (channel stage, IQK, TSSI, DPK, station, data mode) and the first reconcile's role, policy and AFH; the TSSI power check on 5 GHz. All 39 `rtw89_*_host.rs` files pass; the 2.4 GHz cases unchanged.

**2026-11-21, the listen-only rule in data mode:** a host case runs the data service admitted on channel 100 for an access point heard there: a CCK rate is `invalid`; a probe request and a frame to another receiver are `failed` and nothing is published; a frame to the access point is sent; after 5 s with no beacon of it, nothing more is sent.

**Not done yet:** a transmission, a calibration and a join on 5 GHz on the Ryzen, which wait for a lawful channel and country (above); the coexistence's TDMA policies for a busy Wi-Fi beside a busy Bluetooth (not ported on 2.4 GHz either, 213-DRV-0052).

## Related

[213-DRV-0032](213-DRV-0032-rtl8852be-wifi.md) (the driver; its section of 2026-11-21 has the measurements and the method), [213-DRV-0052](213-DRV-0052-coexistence-after-association.md) (coexistence), [213-KRN-0122](213-KRN-0122-platform-power-policy-for-the-wlan-device.md) (the platform's power policy).
