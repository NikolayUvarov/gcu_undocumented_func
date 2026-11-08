# RTL8852BE packet-mode coexistence processing

This DRV-0032 continuation keeps the actual normal/no-link/no-LPS BTC context
when packet DMA becomes active. It is a source-supported runtime prerequisite,
not evidence of physical Wi-Fi, associated-role policy, a Bluetooth service or
a new calibration grant. Startup and RFK admission keep their stricter rules.

## Source contract

The pinned Linux source is `05f7e89ab9731565d8a62e3b5d1ec206485eeb0b`, using
its BSD-3-Clause option and the existing Realtek notice. In
[`coex.c:4480–4520`](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/coex.c#L4480),
BT RFK RUN postpones antenna selection without changing the cached selection.
REQ is not a normal-runtime antenna rejection. The no-link action at 5130 and
`_run_coex` at 7491–7696 still run OFF_BT policy and `_action_common` at 5583.
By contrast, `_chk_wl_rfk_request` at 7476 separately rejects RUN or REQ before
WLAN calibration. The implementation preserves this distinction.

Scoreboard dispatch at 8779 calls `_update_bt_scbd` at 7384–7448 immediately.
The latter clears prior BT information when BT re-enables, then subsequent
BTINFO/version events update that new state. Packet-mode receipt therefore
reads actual `0xac` through the current bounded Window at each scoreboard
event, applies the reset in that position, and only then handles later events.
A snapshot read at the end of the ACK wait would incorrectly erase later
valid information. RUN becoming clear is checked on future calls even without
another BTINFO event.

## Owned transport and progress

The existing Context is never reconstructed. A private transport connects its
shared command/common-action logic either to the actual startup Session or to
the consuming DataSession, with the actual queue, filter and radio identity.
Packet mode never requests the startup ReceivePermit. Known BTC records use a
pure state reducer while the single normal H2C writer waits for ACKs; the
reducer issues no nested H2C command. Accumulated IGNORE_WLAN state and complete
TDMA/slot/version observations retain receive order. Malformed/overflow reports
preserve the exact error and consumed descriptor copy before terminal cleanup.
Unknown non-BTC control records keep the separate explicit inbox boundary.

The source enum in
[`core.h:194`](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/core.h#L194)
identifies RXQ type 1 as PPDU status. Both startup and packet mode consume and
count this auxiliary record after descriptor bounds and the complete payload
copy have been validated. Its payload is deliberately uninterpreted: this is
not a PPDU parser or RSSI guarantee. RPQ, type 7 and unknown control records do
not gain this treatment. Ordinary report bursts cannot fill the control inbox.

Reconciliation runs at packet/command/filter boundaries inside the same
absolute caller deadline, including a pending raw TX deadline. It reads current
scoreboard state, updates antenna/common actions where applicable, and processes
at most eight RX pairs for a changed state. Valid continuing traffic or further
BT changes yield deferred work for a later call; they do not require the rings
to become empty and do not exhaust an eight-iteration convergence retry. BT RUN
returns DeferredAntenna without writing grants, while common actions can still
progress; a later bounded call retries it. The existing actual H2C completion,
register faults, policy-report contradictions and timeouts remain explicit
errors. WHQL is outside this selected normal-mode profile.

Runtime commands retain their 100 ms upper bound. A service must not silently
replace it with a 10 ms timeout and treat a valid 15 ms ACK as failed hardware.
The caller still controls its aggregate deadline; no command starts a fresh
unbounded allowance. `btc_events`, `btc_deferred` and `ppdu_reports` are saturating counters.
Immutable channel, physical MAC and current-filter getters expose observations
without returning the startup owner or creating a new hardware permit.

## Limits and validation

The actual MACID role remains NO_LINK. Selecting a BSSID or receiving a frame
does not establish the upper WLAN service's association/scan state; connected
coexistence policy needs that explicit input. The separately documented
[serial H2C continuation](rtl8852be-h2c-runtime.md) permits sustained commands
only after the actual idle packet-owner transition; startup remains bounded.
Native endpoint binding, reset/reconnect and physical
under-load behavior remain open.

Twelve new cumulative cases compile against the real cold firmware, PHY,
calibration, station and active packet owner. They cover twelve valid BT events
during an ACK wait with ordinary Wi-Fi and TX reports, RUN deferral and release,
REQ without false calibration permission, continuous valid Wi-Fi/BT progress,
scoreboard enable/disable, malformed-event retention, actual common-command
failure, immutable observations, ordered re-enable plus later BTINFO/version,
and a real modeled ACK delayed by 15 ms. PPDU bursts are interleaved with
pending ACKs, ordinary frames, BT events and actual TX reports. Two startup PPDU
cases use the same production reader, including twelve reports before actual
receipt/done ACKs with and without the inbox, and preserving RPQ/type-7/unknown
control records. Execution follows own commit/build and raw publication; these
are models, not hardware guarantees.

After raw publication `173d8620db5fbdb4c9bb9b12588ce29643736f4d`, source
`f7f808b6` passed all twelve runtime and both startup PPDU cases on its own
branch. Complete affected host harnesses passed: data mode 278, BTC session
243, BTC control 11, BTC init 243, PHY 348, IQK 243, DPK 243, channel RFK 250,
station 265, calibration policy 255, post-MAC 243, TX 243 and initial PHY 248.
These overlapping harness counts are not independent case totals. There were
no failures; each broad harness retains its existing one private-firmware
ignored case, and control has none. Commands were `rustc --edition=2021 --test
tests/rtw89_<name>_host.rs` and the resulting binaries with `--test-threads=1`,
on Linux x86-64 with rustc `1.101.0-nightly (c36f14571 2026-10-01)`.
The own native release build passed in 3.80 s with only the inherited unused
initial-PHY fields warning. This is host-model evidence, not a full gate or
physical radio validation.

## The connected station (213-DRV-0052, 2026-11-08)

**What changed.** Before, the data mode kept the startup's `NoLink` policy and role all along. Now its first coexistence reconcile tells the firmware of the station on the admitted channel, as Linux does at `BTC_ROLE_MSTS_STA_CONN_END` (`Context::connect`).

**What it sends, in order:**
1. **ROLE.** `driver_station(channel)`, function 5, 66 bytes, no ACK: `01 40 01 01 02 00 01 00 01 <ch>` and 56 zeros. That is one connection, link mode `2G_STA`, the station bit, then the role connected on port 0, 2.4 GHz, 20 MHz, role `STATION`. Linux's `rtw89_fw_h2c_cxdrv_role_v2` with `fwlrole` 2.
2. **The policy `Station`,** Linux's `OFF_BWB1` (`_action_bt_idle`, Wi-Fi idle). TDMA stays off, and only the OFF slot changes: `(100 ms, 0xea5a5a5a, mixed)` instead of `NoLink`'s `0xe5555555`. The delta is `01 0a 01 00 64 00 5a 5a 5a ea 00 00`. It goes without `policy()`'s drain, as the rest of the runtime: the data mode's rings need not run empty under traffic.
3. **AFH.** `afh_station(channel)`, function 10: `01 <ch> 20`, the same six-channel guard as no-link's `00 00 20`.
4. **Path B's standby** (`s1_standby`). RF path B gets `0xef←0x80000, 0x33←1, 0x3e←0x31, 0x3f←0x179, 0xef←0`, when Bluetooth is on (`__rtw8852bx_btc_wl_s1_standby`).
5. **The WLAN scoreboard:**
   - bit 13 (high LNA) for a linked station with BTG, unless Bluetooth's own bit is set;
   - bit 12 (Wi-Fi's receive over Bluetooth's scan) while linked.

**What stays as it was:**
- **The antenna:** Linux's `BTC_ANT_W2G` is the same connected as not.
- **Freerun** never applies: RFE 1 shares the antenna, with Bluetooth on path B.
- **Leaving:** Linux too keeps the connected policy after a disconnect, and returns to no-link only at power-off. Here a data mode that stops ends the driver.

**Not ported (Linux's other branches):**
- `FIX_TD7010` while linking (the 4-way handshake, DHCP);
- `FIX_TD4010ISO_DL/UL` for a busy Wi-Fi (100 unicast frames in 2 s);
- `PFIX_TD5050` for a busy Wi-Fi beside an inquiry. It also needs null-data templates, which this driver does not register.

So a Wi-Fi that is busy during an inquiry keeps `OFF_BWB1`, where Linux would share the time by TDMA.

**Sources:** pinned Linux `05f7e89ab9731565d8a62e3b5d1ec206485eeb0b`:
- `coex.c` (SHA-256 `c38d8a42a57300c0c376217d3472e2a3176760346ab44b8c21cca6b98c63a11c`): `_action_bt_idle` 4786–4824, `rtw89_btc_set_policy_v1` 3993–4441, `_set_bt_afh_info_v0` 3244–3411, `_set_bt_rx_agc` 5511–5545, the version entry `{RTL8852B, (0,29,29,0)}` 216–222;
- `fw.c` `rtw89_fw_h2c_cxdrv_role_v2` 5132–5213, with `fw.h` (`bae6675fc95c3f6edd348aaa78c2bbb5cc7768f1961a91fb95ff49624aa9e345`);
- `rtw8852b_common.c` 1888–1901.

**Host tests:**
- `connected_station_role_afh_and_policy_match_linux` checks the three bodies against Linux's worked values;
- `runtime_tells_the_coexistence_of_the_connected_station_once` checks that the data mode sends them once, on channel 6.

These are models, not the hardware.
