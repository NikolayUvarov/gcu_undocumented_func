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
