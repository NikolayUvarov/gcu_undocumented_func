# 213-DRV-0087 — The RTL8852BE on 5 GHz: 20 MHz channels at 802.11a's rates

**Type:** driver (`rtw89_wifi`, its tables and their generators) · **Owner:** `DRV` · **Priority:** P1 · **Status:** open (2026-11-26: the join on channel 100 run under `00` on the Ryzen: the access point heard and chosen, one fault mended, the admission then refused by TSSI's power floor under the unknown country's cap; the maintainer's word on that cap awaited) · **Blocked by:** one run on the Ryzen; NET's way to keep the station under `00` · **Main task:** [213](213-amd-ryzen-pc.md) · **Constitution:** MC-6.1, MC-12.1, MC-12.3

## Problem

The driver was brought up on the narrowest configuration, as [213-DRV-0032](../issues-done/213-DRV-0032-rtl8852be-wifi.done)'s plan said: 2.4 GHz, 20 MHz. Every table and calibration in it is that band's.

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

- [x] Host cases cover the 5 GHz channel setting, the policy and its limits, and that nothing is sent on a radar channel before a beacon is heard there. The generators reproduce the tables.
- [ ] On the Ryzen the scan lists the network on channel 100, the station joins it, and 6 MB arrive at a recorded rate. This holds for that machine, that access point and that channel (MC-12.1).
- [x] The Ryzen still joins on channel 8 with the same build, and its rate there is recorded (90699adaa: 289 to 303 KB/s; one run, one machine, MC-12.1).
- [x] The issue says what was not done: other channel widths, HT and VHT, other machines.

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

**2026-11-21, 15:53-15:58 UTC, the branch's head on channel 8** (90699adaa's `rtw89_wifi`, no bench delay, `replace`d into release 202611211156): the station applied RU, scanned the 13 channels and joined CoolNet2 on channel 8 (`DATAPATH ADMITTED ON CHANNEL 8`, DHCP); 6 MB from the LAN three times at 289, 292 and 303 KB/s, the sender's figures. The release's driver was put back after it.

**2026-11-21, the signal wired into both services** (host cases; see the section below): the station gets each frame's signal in the header's byte 2 when its PPDU status report follows it, and the reports that cannot be matched are counted. All 40 `rtw89_*_host.rs` files pass; the full `host_tests.sh` passes.

## Where it stands (2026-11-21, evening)

| Part | State | Host cases | On the Ryzen |
|---|---|---|---|
| Band model, 41 channels, `info` (bands 7, `supported`) | written | `rtw89_receive_service_cases`, `rtw89_service_cases` | the station saw 41 channels |
| Country rules on 5 GHz (DFS and NO-IR listen-only; WMM, indoor/outdoor out) | written | `rtw89_country_host` (UZ, US, 00, RU, DE) | RU applied: no 5 GHz channel, as the catalogue says |
| 5 GHz power tables and their generator | written | `rtw89_channel_policy_host` (Linux's slots; Linux's `txpwr_table` on channel 100), `rtw89_power_tables_test.py`, `--check` | not sent at |
| Power image on 5 GHz | written | `rtw89_channel_power_host` | written on every passive tune |
| Channel stage on 5 GHz, BTG back on 2.4 GHz | written | `rtw89_receive_service_cases` (100 and back to 6; 36, 64, 144, 165) | 40 passive tunes, none failed; beacons heard on 36-157, CoolNet2 on 100 |
| Listen-only rule (receive-only and data mode) | written | `rtw89_receive_service_cases`, `rtw89_service_cases` | not reached: no listen-only channel was allowed |
| IQK, TSSI, DPK on 5 GHz | written | `rtw89_iqk_cases`, `rtw89_tssi_cases`, `rtw89_dpk_cases`, `rtw89_tssi_tables_test.py`, `--check` | not run: nothing may be sent on 5 GHz here |
| Coexistence on 5 GHz | written | `rtw89_btc_codec_cases`, `rtw89_btc_runtime_cases` (the whole admission on channel 100 in the models) | not run |
| The signal in the receive header (both modes) | written | `rtw89_ppdu_host`, `rtw89_receive_service_cases`, `rtw89_service_cases` | a8f70c7a1 on channel 8: every kept frame measured (59 in the scan, 20031 in data mode), none late; `wifi status` −64 dBm |
| A join and a download on 5 GHz | blocked by the country and the access point's channel (above) | — | — |
| 2.4 GHz with the same code | unchanged path | every 2.4 GHz case as before | joined channel 8; 159/242/215 KB/s against the release's 184/262/277 in the same minutes; the head's build (90699adaa, no bench delay) 289/292/303 KB/s at 15:54-15:57 UTC |

**What a person must do for a 5 GHz join here:** choose the Ryzen's country if it is not RU (`wifi set-country XX`, which the station keeps), and put CoolNet2's 5 GHz network on a channel that country lets a station send on (UZ: 36, 40, 44 or 48). With the signal now in the header (host cases only), the station ranks CoolNet2's two networks by it. On the bench their beacons were 1 to 2 dB apart (5 GHz −63, 2.4 GHz −64 to −65 dBm), too close to choose by, so for that run a driver build whose `supported` leaves 2.4 GHz out is still needed (a bench build, not committed).

**Not done, each a later task:** other channel widths, HT and VHT, aggregation; DPK tracking and TSSI tracking on 5 GHz (not on 2.4 GHz either); the coexistence's TDMA policies; other machines than the Ryzen.

## The signal: what the hardware reports, read on the Ryzen (2026-11-21, 16:29-16:33 UTC)

- **Captured:** a bench-only build (release ABI 35, not committed) printed the RX buffers of CoolNet2's beacons and frames to this radio, and the two PPDU status reports after each, into the system log; kept in the owner session's handoff directory (`wifi/ppdu-capture.txt`, `ppdu-capture2.txt`).
- **What they show:** each frame's report follows it at once, with the same 3-bit PPDU count and the same rate in its descriptor (Linux's `rtw89_core_rx_ppdu_match`). The report carries the MAC info (one user, the PLCP) and then the PHY status: its header's average and per-path RSSI, and for a CCK or OFDM PPDU type the IEs whose RPL Linux's 8852B moves the paths by (`convert_rpl_to_rssi`).
- **The values, by Linux's rule** (`RTW89_RSSI_RAW_TO_DBM` of the stronger path): CoolNet2's 5 GHz beacon on channel 100 −63 dBm (header average −64); its 2.4 GHz beacon −64 to −65 dBm; its data frames to this radio −56 to −62 dBm. Linux on the same spot at other hours: beacons at −63 dBm on 2.4 GHz and −67 dBm on 5 GHz.
- **`ppdu.rs`** parses a report so, every length checked. Host cases (`tests/rtw89_ppdu_host.rs`) use five of the captured reports: 5 GHz −63, 2.4 GHz CCK −65, an OFDM break −64, a data frame −61, another station's CCK −75. Every truncation, a longer packet, a cleared valid bit, five users and an IE of no length are covered.
## The signal in the station's header (2026-11-21, host cases only)

**How it is wired:**
- **The session** numbers the receive queue's Wi-Fi frames and hands each PPDU status report to the packet's handler in the queue's order, with the number of the Wi-Fi frame just before it (none if another report came between). A C2H event between a frame and its report does not break the pair; a Wi-Fi frame or a report between does.
- **The match** (`ppdu::Matcher`): a report gives its signal to the one frame its owner kept last, only when that frame is the Wi-Fi frame just before the report and has the report's 3-bit PPDU count and its rate (Linux's `rtw89_core_rx_ppdu_match`, which compares the count and the rate too). Anything else is dropped and counted, never put on another frame:
  - a report of a frame that was not kept (the filter dropped it, the buffer was full, a calibration's poll took it);
  - another PPDU's count or rate;
  - a report after another report;
  - a report that does not parse.
- **The receive-only service** writes byte 2 of the frame it stored, while the station has not taken it yet.
- **The data mode:** the session marks its inbox record. The service's drain leaves a frame that still waits for its report in the inbox while the last poll gave a packet, as Linux's `rx_queue` holds a frame until its report. The next poll gives the report or finds the ring empty, and only then is the frame copied, with its byte 2, for the station.
  - The drain polls until the ring is empty anyway, so this adds no poll.
  - A frame held when a step's 64 polls ran out waits for the next step.
- **What a report finds after the station took its frame** is counted as late, and that frame keeps its 0. This can happen only when the ring was read empty between the frame and its report, so the report was not yet written when the poll looked:
  - in receive-only mode, also when a step's 8 polls ended on the frame;
  - in data mode, the frame then went on at that empty poll.
- **How often that happens under a download, on the Ryzen** (a8f70c7a1, channel 8 under RU, 2026-11-21 18:28-18:35 UTC, four 6 MB downloads): never.
  - The scan: `SIGNAL ON 59 OF THEM, 0 REPORTS AFTER THE STATION TOOK THE FRAME, 79 UNMATCHED`.
  - Data mode, the last minute's line: `ON 20031 OF 20031 FRAMES KEPT; 0 REPORTS AFTER THE FRAME WAS HANDED ON, 237155 UNMATCHED`.
  - The unmatched are the reports of the channel's other traffic, about 650 a second, twelve for each frame kept.
  - This holds for that machine, channel and hour (MC-12.1).
- **The counts that will tell it:**
  - **Scan:** `[RTW89] SCAN: … n FRAMES KEPT, …; SIGNAL ON m OF THEM, l REPORTS AFTER THE STATION TOOK THE FRAME, u UNMATCHED`, once a scan, for the frames the scan kept (beacons and probe responses mostly).
  - **Data mode:** `[RTW89] SIGNAL ON CHANNEL c SINCE THE ADMISSION: ON m OF k FRAMES KEPT; l REPORTS AFTER THE FRAME WAS HANDED ON, u UNMATCHED`, at most once a minute while the counts change.
  - A kept frame with neither a measure nor a late report had no matching report at all; those are `kept − measured − late`.
- **Whether beacons get their figure in time for the scan's ranking:**
  - A scan's beacon is handed to the station only after the step's polls end on an empty ring or after 8 polls. So it carries its figure when its report was in the ring by then.
  - A beacon without one has 0, which `dot11`'s ranking (`best_of`, by weakness) puts last, behind every measured one.
  - On the Ryzen, every one of the 59 frames the scan kept had its figure (above). `wifi scan` then listed each network with its dBm (CoolNet2 −64, the others −64 to −83), where the boot image's driver gives `?`.
- **What it adds to each packet:**
  - **For each Wi-Fi frame:** a counter and an `Option` store in the session, and the choice between two handler calls. For a frame kept, one 4-byte read of its descriptor's word 1 from the copy already made, and a 32-byte store.
  - **For each report:** one comparison of the frame number it follows. Only the report of the kept frame has its descriptor's word 1 read and is parsed (since the run below). The parse reads the MAC info's two words, the PHY header's two and, for CCK and legacy OFDM types, one word per IE (3 to 5 in the captures).
  - **In the data service's copy loop:** one more comparison for each frame.
  - Reports were copied out of the ring and counted before this as well. No register is read, nothing more is copied, and no poll is added. Not measured as time on the bench.
- **Host cases:**
  - `tests/rtw89_ppdu_host.rs` (the matcher, with the captured 5 GHz report): the frame just before is marked −63 dBm, a second report finds nothing, and a frame handed on is late. No frame is guessed after another Wi-Fi frame, another report, another count, another rate, a broken report or a forget.
  - `tests/rtw89_receive_service_cases.rs`: a scan's beacon with the report after it reaches the station with 63 in byte 2 and 6 Mb/s in byte 4. Another PPDU's count, a frame not kept between, and a report after the station took its frame leave 0, and the scan's line counts 4 kept, 1 measured, 1 late, 2 unmatched.
  - `tests/rtw89_service_cases.rs` (data mode, the same cases): the frame waits in the inbox for its report and reaches the station with 63. Alone in the ring it goes on with 0 and its report is late; the minute's line and the data mode's counts agree.
  - Without the inbox's wait, the data mode's first case fails: the frame went on with 0.
  - Frames without reports (every older case) keep 0 as before.

**2026-11-21, 18:28-18:35 UTC, the head a8f70c7a1 on the Ryzen** (release 202611211819, ABI 37, `replace`d in, the boot image's put back after):
- **The country** was RU, announced by the access points: 13 channels to send on, and every 5 GHz channel barred (`wifi channels`: `CHANNEL 100 5 GHZ BARRED`). So no 5 GHz join was possible, and none was tried.
- **The station joined CoolNet2 on channel 8.** `wifi status` showed `36 MB/S -64 DBM`, where the boot image's driver shows `? DBM`.
- **Four 6 MB downloads from the LAN,** the sender's figures: 241, 265, 263 and 211 KB/s. The stack's lines: 203, 216, 200 and 200 KiB/s.
  - Earlier on 2026-11-21: 252 to 353 KB/s in the morning, and 289 to 303 KB/s at 15:54-15:57 with 90699adaa. The boot image's driver was not measured in the same minutes this time, so no difference is shown either way.
  - In that build every report was parsed, about 650 a second, before it was compared. The commit after it parses only the kept frame's report. Its effect on the rate is not measured.
- **The raw lines,** with those NET asked for (the restarts, the country after them), are in the owner session's handoff directory, `wifi/net-lines-37.txt`.

**2026-11-21, boot 57 on the Ryzen (release with 213-DRV-0090's e146cb563, the first 5 GHz join attempts, by accident):**
- **The country:** none was announced in the first scan, so 00 applied: 11 channels to send on, 27 listen-only, channel 100 among them.
- **NET's 550-NET-0090 preference,** now that the signal is reported, chose CoolNet2's access point on channel 100 (−63 dBm).
- **Fourteen tries in 9 minutes, all the same:**
  - `[RTW89] LISTEN-ONLY CHANNEL 100: NOTHING SENT TO A0:36:BC:11:F8:8C, NO BEACON OF IT HEARD HERE SINCE THE TUNE (0 ACCESS POINTS HEARD)`;
  - in the same millisecond `[WLAN] JOINING CoolNet2 ON CHANNEL 100`;
  - 0.9 s later `Failed: NO ANSWER`.
- **No data mode ever started** (no `DATAPATH ADMITTED`). The station sent its authentication within a millisecond of tuning there, before a beacon could come, and the radio refused it by this task's rule. The scan of the same boot had heard that access point on 100 (`HEARD A0:36:BC:11:F8:8C ON 5 GHZ CHANNEL 100`).
- **The wait for the beacon is the station's** (NET, `NET/2026-11-21T1946Z-…`), as is not reading a refused send as "no answer". The driver's rule stays as it is.
- **The trial boot ran out without a join and the machine rolled back.** The bench now runs with `wifi country UZ` chosen.
- **A new line for the first 5 GHz run:** `[RTW89] LISTEN-ONLY CHANNEL n: <bssid> HEARD t MS AFTER THE TUNE, s MS AFTER THE REFUSED SEND`, once a tune, when the receiver of a refused send is heard there. It tells how long the station must wait. The host case is the radar-channel one in `rtw89_receive_service_cases`.

## What a machine running this driver does differently on 2.4 GHz (against `main`'s driver)

- **Each scan takes about four seconds longer** while the country leaves no 5 GHz channel. On the Ryzen under RU (the logs of 2026-11-21): from `[WLAN] Scanning` to `JOINING CoolNet2 ON CHANNEL 8`, 1.35 s with `main`'s driver, 5.6 and 5.7 s with this one. The cause is in `dot11`: `Station::tick` steps through every channel of the radio's list, and gives a channel the country bars (`Allowed::Nothing`) the passive dwell of 150 ms. This driver refuses the tune (`invalid`), so the radio stays on the last channel it was on. 28 barred 5 GHz channels at 150 ms make the 4.2 s. The fix belongs in `dot11`, not here: a scan should skip a barred channel. It is asked of NET in the handoff file `request-NET-WIFI.md`. Until then, every scan, every first join and every rejoin after a stall is about 4 s slower here.
- **The first scan after the station starts anew** (no country yet, so every channel listen-only) visits all 41 channels: 6.3 s on the Ryzen.
- **With a country that allows 5 GHz channels,** the station can join a 5 GHz network. Of two networks of one name it takes the stronger by the signal this driver now reports (host cases only so far; NET's 550-NET-0090 prefers 5 GHz above a signal floor).
- **The receive header's byte 2 carries the signal** (minus dBm), where it was 0, on 2.4 GHz too. In data mode a frame whose report may still come waits in the inbox for at most the rest of the drain, or the next step when the drain's 64 polls ran out. The scan's line has three more counts, and data mode logs one more line, at most once a minute.
- **Nothing else on 2.4 GHz changed in what a user sees:** the join, DHCP and the rate on channel 8 were as before on the Ryzen (289 to 303 KB/s with the head of the branch). The data path is the same code. A 2.4 GHz tune after a 5 GHz one writes the BTG share back, as the initial PHY left it.

## The lawful 5 GHz join: the order of the run (ready for the maintainer's choice)

Needs: the Ryzen's country chosen by the maintainer, and CoolNet2's 5 GHz network on a channel that country lets a station send on (UZ: 36, 40, 44 or 48). Then, under the lease, with a build of this branch on the stick's ABI:
1. `wifi set-country XX` typed on the console (the station keeps it); the log should show `[WLAN] COUNTRY XX (CHOSEN …)` and `[RTW89] COUNTRY XX: n CHANNELS, m LISTEN ONLY`.
2. The driver of the run: a bench build whose `supported` lists the 5 GHz channels alone (not committed), as long as the two CoolNet2 networks are too close in signal for the station's choice. If NET's 5 GHz preference is in by then, the branch's own build.
3. The scan: `[RTW89] SCAN: …` with no failed tune and its signal counts, `[RTW89] HEARD A0:36:BC:11:F8:8C ON 5 GHZ CHANNEL n`.
4. The calibration: `[RTW89] DATAPATH ADMITTED ON CHANNEL n` with no `ADMISSION` stop. A failure line names the stage (IQK, TSSI, DPK, station, data mode) and its register.
5. The join: `[WLAN] AUTO-JOIN CoolNet2: JOINED`, `[NETSTACK] DHCP …`.
6. Three 6 MB downloads, the sender's `TCP_INFO` (`S/srv/putsrv.py`), typed with `get -w --insecure-http -m 60 …`.
7. The same build with `supported` as committed, on channel 8: the join and three downloads.
8. The release's driver back.

**If the chosen channel is a radar (DFS) one,** the listen-only rule held on hardware when:
- no `[RTW89] DATAPATH ADMITTED` line precedes the first `HEARD <the access point> ON 5 GHZ CHANNEL n` of that tune;
- every `[RTW89] LISTEN-ONLY CHANNEL n: NOTHING SENT TO …` names a send refused before that beacon;
- no probe request goes out there: the station sends none on a listen-only channel, and the driver refuses one in both of its modes.

## The join under `00` (2026-11-25): the plan for the Ryzen's next window

The maintainer, 2026-11-25: «То же касается и 5ГГц. Напомню, что на этапе тестирования мы не привязываемся к странам, первый приоритет - чтобы работало, hardening - это отдельный этап». So the join on channel 100 no longer waits for a country that allows the channel, nor for the owner's rule ([550-DRV-0088](550-DRV-0088-the-owners-rule-beside-the-catalogue.md), now the hardening stage's). It replaces the section above for the bench.

**The way: the station under `00`.**
- Under `00`, the rules every country allows, channels 36 to 165 are listen-only (NO-IR or DFS). The station and this driver already send there only to an access point whose beacon they heard on that channel, and send no probe request. Nothing in the driver changes for it.
- The power under `00` is the catalogue's 20 dBm with this driver's cap for an unknown country, 75 % of the maximum (801-DRV-0071).
- **What is missing is NET's:** the station cannot be kept under `00`. `wifi country 00` is refused, and a station under `00` moves to the country the access points announce (RU on the bench, which has no 5 GHz for us). Asked in `NET/2026-11-25T2200Z-DRV-no-country-tie-at-the-testing-stage-…` on `internal-dev-messages`. Until NET's change is in `main`, the bench branch carries a bench-only patch in `wlan`, `dot11` and `keystore` that takes `00` as a chosen country.

**What the code says of the first join there (read, not run):**
- The station tunes and sends its authentication at once. The driver has heard no beacon since the tune, so it refuses that send (`LISTEN-ONLY CHANNEL 100: NOTHING SENT TO …`), and `wlan` drops the frame.
- The beacon comes within about 100 ms (`… HEARD n MS AFTER THE TUNE, m MS AFTER THE REFUSED SEND`).
- The station tries again 300 ms after the first (`ANSWER_MS`), three times in all. The second try is admitted: the driver answers busy, calibrates the channel (on channel 8, boot 93 had `DATAPATH ADMITTED` 100 ms after the scan's last line), and `wlan` holds the frame up to 1 s for it.
- If the 5 GHz calibration takes longer than the tries allow, the join fails once with `NoAnswer` and the automatic join comes again, to a channel already calibrated.

**What may go wrong, and the way back:**
- Nothing was ever sent on 5 GHz by this driver: IQK, TSSI and DPK there, the power tables and the coexistence have host cases only.
- The station takes the network's 5 GHz access point first whenever its signal is above −70 dBm (550-NET-0090) and has no way back to the 2.4 GHz one while that holds. A 5 GHz join that keeps failing leaves the machine without its network.
- So the run is made with the cable at hand: `wifi country UZ` typed there takes channel 100 out again, and the station joins channel 8 as before. The country is kept across restarts, so the same line is the way back after a reboot.

**The order of the run:**
1. The release's start as it is today: the station under the kept UZ, joined on channel 8, the server reached.
2. `wifi country 00` at the console. The log should show `[WLAN] COUNTRY 00 (CHOSEN BY THE USER, KEPT BY KEYSTORE): …`, `[RTW89] COUNTRY 00: n CHANNELS, m LISTEN ONLY`, then the scan with `HEARD A0:36:BC:11:F8:8C ON 5 GHZ CHANNEL 100`.
3. The join: the refused first send and the beacon's line, `[RTW89] DATAPATH ADMITTED ON CHANNEL 100` with its calibration lines and no `ADMISSION` stop, `[WLAN] AUTO-JOIN CoolNet2: JOINED`, `[NETSTACK] DHCP …`.
4. Three 6 MB downloads with the sender's `TCP_INFO`, and the agent's return to the server.
5. A restart: the station comes up under the kept `00` and joins channel 100 by itself.
6. `wifi country UZ`, and the join and three downloads on channel 8 with the same build.

## The first run on channel 100 under `00` (the Ryzen, 2026-11-26)

Release 202611260804 (`main` 6230e1fdb with DRV's pending requests; `rtw89_wifi`, `wlan` and `netstack` are `main`'s), boot 103, the wired card being the default route so that the server stayed reached. It holds for that machine, that access point and that release (MC-12.1). The raw lines are kept by the owner session (`logs-2026-11-26/cable-5g-*.txt`).

**What worked, by the log:**
- `wifi country 00`: `[WIFI] COUNTRY 00 (the world domain) APPLIED, THE RADIO CONFIRMED ITS LIMITS; CHOSEN BY THE USER, KEPT BY KEYSTORE`, `[WLAN] COUNTRY 00 (THE RULES EVERY COUNTRY ALLOWS, CHOSEN BY THE USER, KEPT BY KEYSTORE): 11 CHANNELS TO SEND ON, 27 TO LISTEN ON`.
- **In data mode the scan stays on its channel:** `[RTW89] COUNTRY 00 IN DATA MODE: CHANNEL 8 KEPT AT THE POWER IT WAS ADMITTED AT`, and the station joined channel 8 again. So the order's step 2 alone does not move the station: the radio has to start anew (a restart of the driver or of the machine).
- **The driver restarted once** (`kill`): `[WLAN] THE RADIO RESTARTED (IT HAS NO CHANNEL SET): ITS COUNTRY 00 AGAIN`, `[RTW89] COUNTRY 00: 38 CHANNELS, 27 LISTEN ONLY; APPLIED … COUNTRY UNKNOWN: POWER CAPPED AT 75 % (801-DRV-0071)`, the scan with `[RTW89] HEARD A0:36:BC:11:F8:8C ON 5 GHZ CHANNEL 100` and `SCAN: 37 PASSIVE TUNES, THE LONGEST 4473 US; 63 FRAMES KEPT`, then `[WLAN] JOINING CoolNet2 ON CHANNEL 100`.

**The first fault, mended (c95eb9d51 on `claude/213-DRV-0087-beacons-pass-until-admission`):**
- **Seen:** `[RTW89] LISTEN-ONLY CHANNEL 100: NOTHING SENT TO A0:36:BC:11:F8:8C, NO BEACON OF IT HEARD HERE SINCE THE TUNE (0 ACCESS POINTS HEARD)`, `[WLAN] AUTO-JOIN CoolNet2: NO ANSWER`, try after try, fourteen in all.
- **Why:** the station's filter for its network narrowed the hardware's (`BCN_CHK_EN` and the two beside it at 0xCE20) before the first send. Narrowed, the hardware checks beacons against a BSSID that the receive-only mode never sets, and passes none. On 2.4 GHz the first send admits the channel at once, so nothing waited for a beacon and it never showed.
- **The change:** a scan's filter opens the hardware's at once, as before; a network's narrows it with the send that admits the channel, still before the calibration.
- **With it** (the driver replaced over the cable): `[RTW89] LISTEN-ONLY CHANNEL 100: A0:36:BC:11:F8:8C HEARD 157 MS AFTER THE TUNE, 157 MS AFTER THE REFUSED SEND`, and the next send was admitted. On channel 8 the same build joins as before (`DATAPATH ADMITTED ON CHANNEL 8`, `AUTO-JOIN CoolNet2: JOINED`).

**Where it stops now: `WLANHW SERVICE STOPPED 1: Admission(Admission { channel: 100, failure: Admission(TssiPower) })`.**
- **The numbers** (`calibration_policy::tssi_cap`, a host case printed them): under the world group channel 100's most on one TX is 13 dBm (26 half-dBm); the unknown country's cap (801-DRV-0071, three half-dB steps) makes it 11.5 dBm; the TSSI calibration's bursts go at 12 dBm (the source's raw level 48, `hwdocs/rtl8852be-tssi.md`). 11.5 is below 12, so the admission is refused, and with it every join there. On channels 1 to 11 the capped limit is 13 dBm, which is why 2.4 GHz never met it.
- **The driver then left and came again every 7 s** (`RESTART_AFTER_FAILURE`) until `wifi country UZ` took channel 100 out; the station joined channel 8 at once.
- **This is a rule's matter, not a fault:** the cap is the maintainer's of 2026-11-14. Asked of him on 2026-11-26, with three ways: the calibration held to the channel's world limit while the cap stays for the station's frames (the owner session's recommendation); no cap for a `00` the user chose; or no change.
- **The maintainer's word, the same day:** «Да, делай», to the first. So `calibration_policy::Resolved::resolve` compares TSSI's 12 dBm with the channel's maximum under the world's rules (the cap's `maximum`), and the cap goes on holding every frame the station sends. Host cases: a maximum of 13 dBm admits (channel 6 at that EIRP, and channel 100), one of 11.5 dBm and channel 149's 7 dBm refuse, a known country is as before.

**Not reached yet:** IQK, TSSI and DPK on 5 GHz, the association and any frame sent there. The order's steps 3 to 6 stay as they are, with one change: step 2 is followed by a restart of the radio.

## For a second Linux reference on channel 100 (asked by the owner session)

The first was read with the chip in power save. A second wants power save off and traffic running through the whole dump. Registers to read first (MAC addresses as the driver writes them; the baseband at 0x10000 + offset; RF through `rf_reg_dump`):
- **Width-independent channel setting:** 0xC628; BB 0x0700, 0x0734, 0x2344, 0x4420, 0x4738, 0x4AA4, 0x49C0, 0x49C4, 0x0C6C.
- **The A-band gain tables and offsets:** BB 0x45DC, 0x4660, 0x4664, 0x4740, 0x4744, 0x4748, 0x4694, 0x4778, 0x49B0, 0x49B4, 0x49B8, 0x4A00, 0x4A04, 0x4A08, 0x4ACC, 0x4AD8.
- **TSSI on 5 GHz:** BB 0x0304, 0x120C, 0x12C0, 0x4494, 0x5600-0x5650, 0x5800-0x58FC, 0x5C00-0x5C3C, 0x7600-0x7650, 0x7800-0x78FC, 0x7C00-0x7C3C.
- **Calibration results:** BB 0x8000-0x82FF, 0x9FE0-0x9FEC.
- **RF, both paths:** 0x00, 0x18, 0x10018, 0x1F, 0x20, 0x60, 0x7F, 0x85, 0x8B, 0x8C, 0x90, 0x97.
- **Power:** MAC 0xD200-0xD3FF; Linux's `txpwr_table` and `btc_info` as before.

Linux ran VHT 80 MHz there: SCO, the bandwidth fields, RF18's width and the limits differ from this driver's 20 MHz by design.

## Related

[213-DRV-0032](../issues-done/213-DRV-0032-rtl8852be-wifi.done) (the driver; its section of 2026-11-21 has the measurements and the method), [213-DRV-0052](213-DRV-0052-coexistence-after-association.md) (coexistence), [213-KRN-0122](213-KRN-0122-platform-power-policy-for-the-wlan-device.md) (the platform's power policy).
