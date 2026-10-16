# RTL8852BE initial 2 GHz TSSI engine

This is a private host calibration implementation for the identified cut-1,
RFE-1 RTL8852B, initial 2 GHz/20 MHz, both RF paths. It has no native caller.
Unlike merely programming channel tables, the source TSSI alignment sends
real HT20 MCS7 PMAC packets. A stopped ordinary packet scheduler does not
prevent those transmissions.

## Admission and identity

`tssi::run` requires the same PHY attempt, command session, channel and policy
version as the actual `iqk::Completed`, actual `btc_context::Initialized`,
the channel `Tuned`, and a consumed `CalibrationTransmit(Tssi)`. That opaque
authority is not Clone/Copy. The private [source-profile provider](rtl8852be-calibration-policy.md)
checks actual board/firmware/channel/policy identity before issuing it. A
passive channel cannot match it; the native scoped-input consumer is separate.

The raw manual PMAC levels are 48 and 20 in `R_TXPWR[30:22]`; the source
enables `MAC_SEL_PWR_EN`. Ordinary MAC limit pages alone therefore do not
establish a safe calibration power limit. The exact setter chain now establishes quarter-dBm units, hence requested
12/5 dBm; the [direct source evidence](rtl8852be-calibration-policy.md#direct-source-evidence-for-tssi-units-and-rate)
uses the actual 52B caller and setter, not a similarly named variable or
another BB reference register. The provider checks the actual HT20 MCS7/1TX
source limit and resolved per-path platform caps against 12 dBm. Antenna/system
gain, the real country and device-scoped SAR still require the native input
consumer; no measured physical output or native radio readiness is claimed.

## Source sequence and bounded failure

The controlling source is
[`rtw8852b_rfk.c:3813`](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/rtw8852b_rfk.c#L3813),
with the common chip's actual PMAC/path hooks. The engine performs:

1. An actual outer IQK BTC START grant, then the source IQK RF_AB oneshot
   START. This outer grant is deliberately stricter than upstream's void
   notification call, which can discard a rejected orphan oneshot.
2. Both-path TSSI disable; each path's system, TX-power/HE-TB, DCK, actual
   board thermal table, DAC, initial slope and default alignment settings.
3. Acknowledged zero scheduler and bounded RX-mode wait, then two PMAC
   samples per path: 100 packets, period 5000, source raw powers 48/20.
   Each new CW report is awaited for at most 3 ms and PMAC is stopped after
   each sample. A 30 us ready-clear check refuses a stale report rather than
   treating old calibration data as a new completion.
4. Source signed-nine-bit default alignment and report difference, saved
   BB/path/power restoration, normal MAC selection, and acknowledged zero
   scheduler restoration. There is no cached-alignment shortcut.
5. Both-path TSSI tracking enable, source signed board DE/trim programming,
   matching inner STOP, outer STOP, and final live guard/deadline check.

One 4.5 s absolute work budget covers the whole engine, including callbacks.
Short hardware waits retain their own finite source ceilings. Any failure
keeps the primary stage/error and independently attempts PMAC stop, saved
state restoration, TSSI disable, RF shutdown, scheduler restoration when
the earlier exchange completed, matching inner STOP, and outer STOP. A
failed/uncertain mailbox or notice is never retransmitted. Cleanup has
independent finite budgets; it is not a guarantee against indefinite CPU
preemption or inaccessible hardware.

If PMAC stop cannot be confirmed, RF shutdown is attempted immediately and
saved TX-power/path restoration is skipped (`restore_skipped=true`). The
engine never restores live calibration power or releases BB reset after
an unconfirmed stop. If a stop write reports failure after taking effect,
cleanup must confirm the stopped state before restoring saved registers.
Backing DMA remains owned by the caller throughout.

Successful `Completed` records both CW report pairs and alignment registers,
and reports both `is_tssi_mode` flags true. It does not advance the overall
PHY to WLAN-ready: DPK, remaining MCC/BTC notifications, association, normal
TX/RX integration and real hardware evidence are separate dependencies.

## Reproducible source data and licence

The BSD-3-Clause alternative of pinned Linux commit
`05f7e89ab9731565d8a62e3b5d1ec206485eeb0b` is used. The offline
`scripts/hardware/rtw89_tssi_tables.py` validates all three complete hashes
before extracting 17 exact TSSI tables, the 120-record HT20 MCS7 PMAC table,
and four 30-byte thermal arrays. Every selected initializer must parse
completely with its expected count and validated address/mask/value fields.

| File | SHA-256 |
|---|---|
| `rtw8852b_rfk_table.c` | `ebc6840d2e15c7a9f12fb7d7121ca19bb8acff93524d3902a7ce892e0714fc21` |
| `rtw8852b_common.c` | `34dcc96967f408ba2b7589af26f49e8ba26c0a822934811e599a2974b86a3afb` |
| `rtw8852b_table.c` | `8b9b5fc32628037d8dec86cff976913164723d87abe38fd09d229c57b509b42f` |
| `rtw8852b_rfk.c` | `4613e3d2755f14649154a1c8393bf72ae215e8b683219a0763a033b224c715f` |

No proprietary firmware or device data enters the repository. Native build
passed in 1.21 s without warnings. After code `983e4c72` was published in
`fast-test` merge `b169a2753535d74968635e95740941abda175d0a`, the owner's
exact saved PHY harness passed 183 cases in 1.04 s, including the twelve new
TSSI cases, with zero failures and one existing private-firmware case ignored.
All five Python extractor cases passed, and regeneration from the three pinned
source files matched the committed tables exactly. All six current host
harnesses that include `phy.rs` also compiled without extra imports. The fixtures
model fresh/stale CW reports, actual PMAC writes and sample counts, thermal
bytes, identity/admission refusal, timeout/uncertain callbacks, and both
ignored and applied-but-failed stop writes. They are not RF measurements.
