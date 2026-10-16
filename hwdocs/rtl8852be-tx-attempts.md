# RTL8852B firmware transmit-attempt accounting

Issue: [213-DRV-0032](../issues/213-DRV-0032-rtl8852be-wifi.md).
This follow-up resolves the deliberately unknown count in the
[raw transmit prerequisite](rtl8852be-tx.md). It changes no service interface,
does not activate native transmission and does not claim a Ryzen measurement.

## Primary evidence

The source implementation remains Linux v6.19 commit
`05f7e89ab9731565d8a62e3b5d1ec206485eeb0b`, under
`drivers/net/wireless/realtek/rtw89/`. The relevant exact paths are:

- `rtw8852b.c` selects original `rtw89_core_fill_txdesc` and 24-byte body/info.
- `txrx.h` defines DATA_TXCNT_LMT_SEL and DATA_TXCNT_LMT; `core.c`,
  `rtw89_build_txwd_info1`, packs the selected value without an adjustment.
- `fw.h` defines the original C2H word-five DATA_TX_CNT at bits 13:8.
- `mac.c`, `rtw89_mac_c2h_tx_rpt`, selects that original layout for RTL8852B,
  and finalizes on DONE or the counter reaching the configured limit.
- `pci.c`, `rtw89_pci_tx_status`, supplies separate RPQ ACK/status handling;
  its descriptor release alone does not supply a transmission count.

The ambiguity in the field names is resolved by the upstream patch-author
and Realtek-maintainer discussion. Fedor Pchelkin explicitly describes a
configured limit of one as a single attempt, with returned counter one.
Ping-Ke Shih checks examples ending on attempts one, four and eight; the
follow-up confirms the final failed-attempt boundary and success on attempt
eight. See the [first exchange](https://lkml.iu.edu/hypermail/linux/kernel/2510.1/07376.html)
and [follow-up](https://lkml.rescloud.iu.edu/2510.2/04120.html).
The [later v5 implementation](https://lkml.rescloud.iu.edu/2511.0/03774.html)
also explicitly places RTL885xB in the original V0 report format. The final
pinned source above retains that chip dispatch and terminal rule.

The exact RTL8852BE vendor tree independently matches the layout and terminal
comparison: [HRex39/rtl8852be](https://github.com/HRex39/rtl8852be/tree/68212c16b229080742280f954b3ffde173e63fcc),
commit `68212c16b229080742280f954b3ffde173e63fcc`:

| File | SHA-256 | Relevant fact |
| --- | --- | --- |
| `phl/hal_g6/mac/mac_ax/mac_txccxrpt.h` | `c088bf6a761ed03c23bd8aab02f5f44caeb5c3ff299335bee37d9d26cc5796c1` | Payload word-three count bits 13:8, after the C2H header |
| `phl/hal_g6/hal_api_mac.c` | `0ae0f788e61f8c9e7be930f0d2f93bcca778d8fb387cd3e2f2171c20ee77f571` | `rtw_hal_mac_process_c2h` suppresses unsuccessful reports until the selected limit |

The vendor implementation was inspected for protocol facts only; no vendor
code is imported by this change. Existing Linux-derived code retains its
BSD-3-Clause attribution. Measurements on adjacent USB chips are not used as
RTL8852BE hardware evidence. The upstream discussion concerns common firmware
reports used by USB; this PCIe engine explicitly requests those C2H reports
and additionally waits for its own RPQ and DMA consumption.

## Checked implementation boundary

Each published descriptor uses selected limit one, fixed legacy rate, disabled
rate fallback, no aggregation, no RTS and software-preserved sequence. The
firmware reports actual data-transmission attempts for that descriptor. The
implementation accepts final DONE/RETRY_LIMIT only with counter one, immediately
rejects any counter above the selected limit, and rejects zero-count DONE.
A zero-count non-DONE report is not final and never adds to the result.

After the matching RPQ status, actual descriptor consumption and final C2H
agree, the implementation adds the checked firmware count. `Known(n)` is
returned only for `1 <= n <= caller_limit <= 15`; its source is not a count of
doorbells, descriptor retirement or RPQ alone. Group-address completion reports
one attempt with `acked=false`. Lifetime/MACID-drop, missing reports, malformed
counters and timeouts return failures without an invented completed-send count.
Raw C2H and completed-descriptor counts remain separately available.

The existing ownership/timeout rules remain: DMA is retained on failure, an
unresolved tag is never recycled, and final firmware report ordering is an
explicit device/firmware TCB contract. The strict count result can satisfy the
existing `wlanhw` 1.1 field; native routing, association, link recovery and
physical acceptance remain independent work. No interface-version request is
needed for this source-supported narrow configuration.

## Validation scope

The focused fixtures cover successful first attempt, retries through fifteen,
group-address ACK semantics, all six orderings of the three completion events, intermediate
reports, checked impossible-counter rejection, retained DMA and C2H-only failure.
The existing cold DataSession case checks Known(1) through the actual owned RX
dispatcher. These are model checks of implementation against the protocol;
the model cannot establish physical airtime, ACK reception or firmware timing.
Execution follows own build and raw publication; results are recorded separately.
