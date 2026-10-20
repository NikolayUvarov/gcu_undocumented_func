# RTL8852B initial host DPK

The private `dpk::run` entry implements the initial non-DBCC 2 GHz/20 MHz,
both-path, slot-zero/no-reload DPK transaction for the existing cut-1/RFE-1
profile. It requires the actual channel `Tuned`, same-attempt/session BTC
`Initialized`, successful same-channel/policy TSSI `Completed` and a consumed
`CalibrationTransmit(Dpk)`. The current platform has no production constructor
for that calibration-transmit authority: source DPK raw TXAGC levels still need
an actual platform SAR/EIRP policy. This code does not enable native DPK or
normal WLAN transmission. A channel token or firmware ACK alone is insufficient.

## Pinned implementation and source

The adaptation uses the BSD-3-Clause option of Linux v6.19 commit
`05f7e89ab9731565d8a62e3b5d1ec206485eeb0b`, with Realtek notices retained in
[THIRD_PARTY](../THIRD_PARTY.md) and [the licence](../LICENSES/BSD-3-Clause-rtw89.txt):

| Source | SHA-256 | Used portion |
|---|---|---|
| [rtw8852b_rfk.c](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/rtw8852b_rfk.c) | `c4613e3d2755f14649154a1c8393bf72ae215e8b683219a0763a033b224c715f` | `_dpk_*`, initial `rtw8852b_dpk`, shared RF backup and RXDCK trigger |
| [rtw8852b_rfk_table.c](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/rtw8852b_rfk_table.c) | `ebc6840d2e15c7a9f12fb7d7121ca19bb8acff93524d3902a7ce892e0714fc21` | DPK AFE/restore/KIP tables: 29/24/2 entries |
| [reg.h](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/reg.h) | `a302f390b451ce1dcdf8dd1d5149910a71cb8621b309b274eb5b1ce060335333` | BB/RF fields and completion registers |

The selected chip source has no `fem_setup` callback; its initial FEM state has
no external PA. This bounded entry retains that exact source profile and the
existing actual cut/RFE guard. It does not extend this assumption to another
RFE, another silicon cut or an unknown external amplifier.

`python3 scripts/hardware/rtw89_dpk_tables.py <pinned-rtw8852b_rfk_table.c>`
regenerates the checked-in source tables offline, after checking the source
hash. Adding `--check` compares without writing. No firmware blob or downloaded
source is stored by this helper.

## Transaction and failure handling

One three-second deadline covers initial/final live guards and the transaction.
The engine uses the existing actual `RxdckIo` scheduler mailbox and typed DPK
BTC START/STOP. It first observes an already-zero scheduler and actual initial
DPK backoff (`R_DPD_BF`); the zero-backoff branch additionally verifies both
previously initialized DPD gain-scale fields. Actual Bluetooth arbitration
must finish before AFE or RF calibration writes.

After scheduler pause and bounded RX-mode checks, save the source BB3, RF11
and per-path KIP3 registers and pause active TSSI. Apply AFE, then for each path:
TXAGC, source RF settings, the small DPK RXDCK trigger, KIP preset/RXAGC,
synchronization/DC/correlation checks, bounded digital-gain/gain-loss adjustment,
thermal observation, IDL, result packing and DPD enable. Restore AFE/BB/KIP/RF
and the prior TSSI pause bits, confirm the zero scheduler and release BTC.
The ordinary packet queues remain stopped; calibration itself may radiate.

Each NCTL command has both source completion polls (20 ms and 2 ms), inside the
aggregate deadline. Pending state is set before publication and cleared only
after both observed completions and the final handshake. Timeout or a possibly
applied trigger forbids another trigger. Such an error immediately attempts
independent BB/RF shutdown, before a scheduler cleanup that may wait for its
mailbox. It skips BB/KIP/RF restoration and TSSI resume because NCTL ownership
is unknown; `nctl_uncertain` and `restore_skipped` report that distinction.
DMA and the device remain retained for the outer native shutdown.

With known command completion, failure cleanup independently attempts DPD
disable, saved register/TSSI restoration, zero-scheduler confirmation, radio
off and one matching BTC STOP. Group errors remain separate from the original
failure. The same uncertain register-mailbox exchange is never retried.
Independent cleanup has fixed group budgets; it does not reuse an expired
three-second work deadline. No failed path returns `Completed`.

Two intentional stricter behaviors differ from upstream's diagnostic logging:
NCTL completion timeout fails the transaction, and exhausting the six AGC
adjustments/200-transition cap is not accepted as convergence. Explicit source
upper/lower TXAGC bounds remain valid exits. These changes are not a claim of
full upstream behavioral equivalence.

## Evidence and remaining work

Fourteen model cases exercise the production engine: both source
paths and restoration, actual backoff branch, both NCTL timeouts, signed DC/PAS
and correlation thresholds, AGC saturation/nonconvergence, BTC/scheduler denial,
RX-mode timeout, possibly applied writes, independent cleanup errors, late
completion, failed final STOP and frozen-clock bounds. The fixture models
hardware callbacks; it does not supply a native transmission authority.
The typed outer entry compiles with the actual TSSI and channel token types;
a full real-token cold-start composition remains a separate integration test.
Two table-provenance tests and the focused harness are registered in the host
gate. The focused execution record is below.

Not implemented by this slice: 5 GHz/40/80 MHz, DBCC, coefficient reload,
thermal DPK tracking, native RFK-power policy or real-device calibration-quality
and traffic validation. The complete channel orchestrator also needs actual
connection-RFK notifications, the source 30 ms BT-preservation gaps between
IQK/TSSI/DPK and the final MCC notification; this entry does not fake them.
[213-DRV-0032](../issues/213-DRV-0032-rtl8852be-wifi.md)
remains open.

### Failed final STOP boundary

The first published run of `2fc291a5` on 2026-10-16 passed 13 of 14 DPK
models and failed the final-STOP case. The test incorrectly required even a
failed STOP to be the final MMIO event; independent emergency shutdown must
still occur. The failure also exposed unnecessary AFE/KIP/RF restoration
following a possibly released grant. The correction does not retry STOP.
Once STOP was attempted, only bounded BB/RF shutdown runs; DPD control,
coefficient, AFE/KIP/RF and TSSI restoration and scheduler callbacks are skipped.
`disable_skipped` distinguishes an unattempted, unknown DPD-disable result from
an attempted write failure, and `restore_skipped` records the cleanup refusal.
The primary STOP/deadline error and radio-off result stay separate. Tests retain
strict STOP-last ordering for earlier failures, and require exactly one STOP
plus only radio-shutdown writes after failed or late STOP. DPD enable bits may
remain set in an inactive radio; no working radio or hardware abort is claimed.
The two Python provenance checks and exact pinned-table regeneration passed
on the first run. After source `40addff026369c01da5cc3f4f72654d19f99eb6f`
built and was published in `fast-test`
`c38df1ddff7151af1e1ededf551f736c2f6b6766`, all 14 corrected DPK cases
passed on its own source branch (0.01 s, no failures or ignored cases,
178 unrelated cases filtered). The first failed run above remains part of
the evidence; this focused model result does not establish hardware behavior.
