# RTL8852BE channel and RFK implementation dependencies

This is the remaining implementation plan for
[213-DRV-0032](../issues/213-DRV-0032-rtl8852be-wifi.md), not a claim that the
Ryzen radio can select a channel or transmit. The first implementation target
is single-band PHY0, 2.4 GHz, 20 MHz. This limits code scope; it does not assert
that any particular channel is permitted for the actual board or location.

## Actual Ryzen evidence and missing policy inputs

The [recorded Ubuntu observation](rtl8852be.md#actual-ryzen-linux-observation) establishes
RTL8852BE cut 1/RFE 1. It does not establish the actual MIND-read board country,
channel-plan byte, crystal value, TSSI/thermal/power maps, firmware capability
response, platform SAR or antenna-gain constraints. No successful MIND board
read or calibrated radio operation is recorded here.

The native board diagnostic will log `COUNTRY` as two raw hexadecimal bytes
and `PLAN` as the raw byte. Until that real result is available, no country,
allowed channel list or allowed transmit power is assigned to Ryzen. Test
fixtures containing `US`, a channel-plan byte or a synthetic board are not
device evidence. Host timezone, SSIDs and a bound Linux driver cannot replace
these observations. Read-only Linux regulatory/channel reports can be recorded
separately, but cannot be relabelled as OTP values or MIND runtime evidence.

The pinned source reads the two country bytes in
`rtw8852b_common.c::__rtw8852bx_read_efuse`. `regd.c::rtw89_regd_init_hint`
selects a recognized non-worldwide country as a strict regulatory hint and
ignores conflicting country information elements; a worldwide/unknown board
follows the external 802.11 stack's policy. Its country map selects TX-power
table groups. Allowed frequencies, disabled/no-IR/DFS flags and bandwidth
restrictions also depend on the stack's regulatory rules. The packed
`channel_plan` field is not interpreted by the reviewed 8852B parser/regulatory
path; MIND must not invent a plan-byte-to-channel mapping.

A transmit-capable configuration therefore needs explicit, versioned regulatory
policy provenance, the actual board country relationship, the selected channel
and bandwidth, and applicable platform/antenna constraints. Worldwide or
unknown bytes are not a successful country choice. Missing policy cannot be
replaced by an assumed universally safe channel or power value.

## Required source stages

| Stage | Implementation and prerequisite | Boundary after completion |
|---|---|---|
| Channel/power policy | Validate actual Board and effective policy; select only supported 2.4 GHz/20 MHz, preserve disabled/no-IR restrictions, bind policy and channel to this PHY/firmware attempt. Decode the pinned chip power tables and their exact units. | Validated inputs, no MMIO or transmission. |
| Channel prepare | Source `set_channel_help(enter)`: save/stop scheduler through the sole firmware mailbox, stop PPDU reports, pause both TSSI paths, disable ADC, delay 40 microseconds, apply band-specific BB reset. | Packet TX still stopped; all changed state saved for bounded cleanup. |
| MAC/BB/RF tuning | Source `set_channel`: MAC channel fields, CCK/SCO, BB channel/bandwidth/gain/masks/reset, then both RF channel/bandwidth paths and RXBB LUT. Include source RF lock/readiness checks and close temporary LUT write gates. | Tuned registers only; not calibrated or ready for packets. |
| Channel power | `set_txpwr`: by-rate values, rate offsets, region-dependent TX shape, ordinary and RU limits, antenna/SAR path-power difference. Complete before channel-done or any calibration-generated transmission. | Actual bounded power programming with readback; no scheduler release. |
| Channel done | Source PPDU/ADC/TSSI/BB restoration and original scheduler mask. Initial bring-up permits only the verified original zero mask; never invent a new enabled mask. Handle band-switch BTC and source TSSI scan transition. | Initial tuned state; remaining RFK still mandatory. |
| Channel RFK | Source `rfk_channel`: record MCC channel, connection-RFK begin, RXDCK, IQK, preserve Bluetooth time 30 ms, TSSI with `hwtx_en=true`, preserve 30 ms, DPK, connection-RFK end, RF MCC H2C notification. | Channel calibration results, with explicit faults; association is still separate. |
| WLAN frame service | Station/MAC role and CAM state, actual RX data parsing and TX descriptors/rings, channel selection/scan policy, authentication/association and existing WPA2 integration. | Only physical traffic validation can establish DHCP/ping/Effector connectivity. |

The exact 8852B `rfk_channel` dispatch does **not** call TXGAPK. IQK and DPK
each use their own BTC RFK START/STOP and scheduler stop/restore around the
source procedure. TSSI uses source one-shot BTC notifications and may generate
calibration transmissions; it must not be treated as a harmless receive-only
stage. Existing initial RXDCK code needs a channel-stage entry with the same
real scheduler and BTC ownership, not a fabricated success callback or a
reset of PHY state to bypass its current state machine.

## Power inputs that cannot be skipped

`rtw8852b_common.c::__rtw8852bx_set_txpwr` orders the writes above.
`phy.c::rtw89_phy_read_txpwr_limit` combines chip/RFE table limits, optional
antenna-gain offsets, SAR and the applicable TPE constraint. RTL8852B uses
MAC half-dBm, RF quarter-dBm and BB eighth-dBm units; conversions and signed
clamps need boundary regressions. Its chip flags support per-antenna SAR and
antenna gain, but not TAS or 6 GHz.

The default chip data in `rtw8852b_table.c` includes by-rate, TX-shape, 2G/5G
ordinary and RU power tables. Existing BB/RF table extraction does not import
these power tables. Import only the required reviewed subset with source hash
checks and BSD attribution; retain the difference between an absent entry and
a valid zero value. Upstream's explicit worldwide fallback is a chip-table
fallback, not independent regulatory authorization.

`sar.c` distinguishes no configured SAR source from common/ACPI policies and
can return the source maximum when no policy is configured. MIND must record
the actual platform-policy result before representing it as absence; an
unimplemented reader is not an observed absence. Per-path constraints affect
both final limits and the A/B reference difference. The initial zero-reference
writes in the DM tail do not replace any of these channel-dependent limits.

## Work and evidence order

Implement and review the policy/table decoder and bounded channel lifecycle
before IQK/TSSI/DPK integration. Host cases must cover actual policy mismatch,
unknown board country, disallowed/no-IR channels, signed power-unit boundaries,
source register order, every applied write failure, busy indirect ports,
deadline expiry, independent cleanup and no restoration after an uncertain
mailbox exchange. Tests cannot substitute for the missing actual board/policy
observations. Each code slice follows own build, raw publication, then tests;
native activation waits for the real prerequisite chain.

## Pinned primary source

All source references use Linux commit
`05f7e89ab9731565d8a62e3b5d1ec206485eeb0b` and the BSD-3-Clause alternative
where code/data are adapted. Existing [PHY source hashes](rtl8852be-phy.md#exact-source-and-table-selection)
cover `core.c`, `phy.c`, `rtw8852b.c`, `rtw8852b_common.c`, RFK and tables.
Additional read-only source files cached for this dependency review are:

| File | SHA-256 |
|---|---|
| [regd.c](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/regd.c) | `1d7397fecb4716c6053db7eb9d032483445f41da7200166d4a7e4be4cb1411b7` |
| [sar.c](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/sar.c) | `7fa1de982974b878d3e2e68228f6c3b230080ff083074ff91d53a2c0e85acdf3` |
| [sar.h](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/sar.h) | `e135ce148d1e36a9c683476a87b43a8a9b9b568bb33add270c0de7b86edf8147` |

These references describe driver behavior, not a jurisdiction-specific legal
determination or current regulatory database. No new regulatory table or
firmware payload is included by this document.
