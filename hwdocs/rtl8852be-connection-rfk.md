# RTL8852BE initial channel coexistence notifications

This private startup slice serves 213-DRV-0032. It connects the existing owned
`btc_context` and `Session` to the channel calibration coordinator; it does not
supply calibration transmission authority or activate a WLAN endpoint.

## Source and supported profile

The source is Linux v6.19 commit
`05f7e89ab9731565d8a62e3b5d1ec206485eeb0b`, under the BSD-3-Clause option already
recorded in `THIRD_PARTY.md` and `LICENSES/BSD-3-Clause-rtw89.txt`:

- [coex.c](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/coex.c):
  `rtw89_btc_ntfy_conn_rfk`, `rtw89_btc_ntfy_preserve_bt_time`, and the
  `btc_btinfo_lb2.a2dp` field used by `_update_bt_info`.
- [rtw8852b.c](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/rtw8852b.c):
  channel RFK ordering, including two conditional 30 ms Bluetooth windows.
- [fw.c](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/fw.c) and
  [fw.h](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/fw.h):
  `rtw89_fw_h2c_rf_ntfy_mcc`, its feature selection and normal packet identity.

Only the established cut-1/RFE-1, normal firmware 0.29.29.15, initial
2 GHz/20 MHz, no-link/no-LPS session is supported. The caller verifies actual
PHY attempt, channel/policy and `Initialized` identity at stage boundaries.
The owner separately verifies its original command session and live RX state.
There is no new normal-H2C writer or sequence allocator.

## Actual owner behavior

`Session::connection_rfk(true, budget)` changes the owned host state from fresh
to active. This is the source `con_rfk` flag, not an H2C or an RFK grant. In the
current no-link profile, changing it alone requires no policy command. The
source's linked/scan policy branches are outside this slice. A repeated initial
connection attempt is refused; this first-channel-only contract does not invent
MCC history for a later channel.

`Session::preserve_bt_time(budget)` requires active connection calibration and
no pending RFK attempt. It first consumes bounded, validated C2H state. A2DP is
BTINFO byte 2 bit 6, not the presence of any Bluetooth profile. If that actual
snapshot has A2DP, the method waits until 30,000 microseconds have elapsed on
the monotonic clock, polling RX and processing retained BTC events between
bounded waits. A later event does not shorten the snapshot's requested window.
Without A2DP it returns after live validation. Frozen/regressing clocks, an
insufficient overall budget, late wakeup, malformed messages and event overflow
return errors, never fabricated elapsed time. Subsequent RFK START still
performs actual arbitration and common-policy reconciliation.

`connection_rfk(false, budget)` clears the matching host active state before
checking a poisoned session or clock. Its error remains visible; it does not
rehabilitate the session. The coordinator must perform independent radio
shutdown after calibration failure and attempt this matching END exactly once.
An outstanding RFK attempt still causes refusal; clearing this connection flag
does not implicitly stop or grant RFK.

`Session::notify_mcc(channel, budget)` is permitted only after END, with no
pending RFK and a healthy owned command/RX session. For first slot zero and
channels 1 through 13 the exact RTL8852B payload is five little-endian u32 words:
`[channel, channel, 0, 0, channel]`. RTL8852B does not select the newer
`RFK_NTFY_MCC_V0` feature. The normal H2C identity is category 2, class 0x0a,
function 2; explicit receive/done ACK flags are false, while the existing
engine's every-fourth-command receipt remains active. Completion includes
command DMA drain and any requested receipt, not a claim of applied RF state.
Uncertain publication, missing required receipt or DMA ownership loss poisons
the session and cannot be retried. No transmission permission follows from it.

Each callback accepts at most 100 ms and preserves one absolute deadline through
register access, RX, command publication and final validation. This is a bounded
startup sequence, not a continuous coexistence implementation. The existing
256-command normal-H2C limit remains explicit.

## Verification boundary

Ten additional cases in `tests/rtw89_btc_session_cases.rs` use actual completed
firmware/queue/RX tokens and the existing production Session fixture. They cover
owner/state refusal, END after failure, A2DP versus other profiles, queued and
later BTINFO, genuine elapsed time, frozen/regressing/late clocks, pending RFK,
malformed input, MCC bytes and periodic ACK, and no retry after uncertain send,
missing receipt or busy DMA. After source
`bf6747f58975f9ba15b73d06948f3a32819d16cc` built and was published in
`fast-test` `c38df1ddff7151af1e1ededf551f736c2f6b6766`, all 62 Session
cases, including these ten, passed on the own source branch (2026-10-16,
0.29 s; no failures or ignored cases, 115 unrelated cases filtered).
These are host models with a reseeded register handoff, not hardware evidence
or the full channel coordinator's cumulative cold-state test.
