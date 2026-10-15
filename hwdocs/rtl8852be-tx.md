# RTL8852B raw MPDU transmit prerequisite

Issue: [213-DRV-0032](../issues/213-DRV-0032-rtl8852be-wifi.md).
This private library moves management and ordinary/QoS data MPDUs through
band-zero ACH0..3 and CH8. It is not yet a `wlanhw` service. Its start transition
has no production caller: actual channel RFK/policy, station CMAC configuration,
live session ownership and ordinary scheduler activation remain prerequisites.
No ABI, IDL, native grant or live radio configuration changes here.

## Primary source and descriptor contract

The selected BSD-3-Clause source is Linux v6.19 commit
`05f7e89ab9731565d8a62e3b5d1ec206485eeb0b`,
`drivers/net/wireless/realtek/rtw89/{core.c,core.h,pci.c,pci.h,txrx.h,mac.c,mac.h,fw.h}`.
`rtw8852b.c` selects the original `rtw89_core_fill_txdesc`, 24-byte body,
24-byte info and original PCI address/report formats. A normal descriptor is
that 48-byte WD followed by eight bytes of WP information and one eight-byte
address entry. The TXBD references those 64 bytes; the address entry references
the unchanged MPDU, without FCS. PCI report sequence is the 15-bit WP tag,
not the 802.11 sequence or firmware's four-bit diagnostic report number.

`txdesc.rs` validates length 24..2346, frame version/type/header bounds,
the twelve legacy rates in 500 kb/s units and a descriptor-submission limit 1..15. QoS priority
selects the actual source AC/TID mapping. Control frames, A-MSDU and non-normal
QoS ACK policies are explicitly unsupported in this initial slice; they are
not silently treated as normal acknowledged data. Software encryption bytes
are preserved and hardware encryption is disabled. The raw caller's sequence
is retained by disabling hardware sequence substitution. The chip appends FCS.

## Actual status and software retries

RPQ reports contain status, QSEL, MAC ID and WP tag, but no attempt counter.
The independent MAC C2H diagnostic uses category 1, MISC class 9, function 1;
its 32-byte payload carries four-bit SW_DEFINE in word 2 and six-bit data TX
count in word 5. A pure codec preserves it, but the engine does not depend on
that smaller correlation space or infer a count from DMA consumption.

Each WD selects DATA_TXCNT_LMT=1, a fixed legacy rate with DISDATAFB, no
aggregation and no RTS/HW_RTS. The pinned source packs that field directly
(`core.c::rtw89_build_txwd_info1`) and compares firmware TX count to the selected
limit (`mac.c::rtw89_mac_c2h_tx_rpt`), but does not explicitly settle whether
one means one total transmission or one retry. An independent
[OpenIPC primary measurement](https://github.com/OpenIPC/devourer/blob/master/docs/scheduled-mac.md#unicast-ack--txreport-capability-matrix)
reports total-transmission semantics on RTL8832CU. That is supporting evidence
on a different chip, not RTL8852BE hardware evidence.

Accordingly every `Completion` retains the actual RPQ `Status`, ACK result,
completed-descriptor count and **`AttemptCount::Unknown`**. It cannot satisfy
`wlanhw` 1.1's exact on-air attempts contract, and no service adapter or native
start is exposed. The optional firmware codec also names its value
`raw_data_tx_count`; it is not silently substituted for a final correlated
on-air count. Resolving this boundary requires exact-chip primary semantics
or independent physical observation before the adapter can open.

The engine advances its descriptor count only after both the matching RPQ
status and PCI read-index consumption. DONE ends the request; a group address
always returns `acked=false` and is never resubmitted. RETRY_LIMIT permits
another descriptor up to the caller's explicit submission limit, preserving
sequence and setting the MPDU Retry bit. This is not yet a promised bound on
on-air attempts. LIFE_TIME/MACID_DROP retain raw status and the count of earlier
completed descriptors, then require shutdown; they do not manufacture a
transmission count. The source and tests distinguish descriptor completion
from both DMA-only completion and actual attempt accounting.

## Ownership, limits and shutdown

The caller supplies 135,168 disjoint coherent WB bytes below 4 GiB on x86-64,
with strongly ordered UC MMIO and no aliases. Five 32-entry TXBD rings occupy
the first page; 32 separate 4 KiB slots hold each WD and payload. Only one
logical MPDU is outstanding. This is deliberate for the synchronous `send`
contract; there is no per-packet heap allocation. RXQ routing stays with its
session owner; each poll consumes at most one RPQ descriptor through the real
owned RX engine, copying data before the RX ring returns ownership.

A payload is immutable until its actual RPQ release and descriptor consumption
are both observed, following `pci.c::rtw89_pci_release_txwd_skb` and
`rtw89_pci_reclaim_txbd`. The source's extra eight-packet retention applies to
FWCMD, not these RPQ-released payloads. Release/Acquire fences constrain CPU
ordering; x86 coherent WB/UC ordering supplies the device-visible ordering.
The allocation is retained even after errors, stop or object drop; only
external verified PCI quiescence permits its owner to free it.

WP tags rotate at 15 bits and descriptor indices at 32 only after earlier
ownership is resolved. This follows the device's release-report protocol,
which does not provide an unbounded generation ID. A recent exact duplicate
is harmless; a wrong tag/QSEL/MAC ID, polluted or conflicting report poisons
the session. A timeout or uncertain doorbell never permits later reuse or
restart. Thus unresolved late reports cannot be carried across tag rollover;
an arbitrary device replay from an already completed older generation is not
a hardware guarantee established by this source or these tests.

Each request has an absolute 1..1,000,000 microsecond caller budget, checked
before publication and after final completion. Clock regression fails closed.
Each poll has finite work, and at most `timeout_us` polls are admitted even
if the clock freezes. All counters check overflow. The independent stop sets
only owned TX/WP stop bits and checks busy state under fresh bounded budgets;
it does not disable CH12 or RX, restore calibration, free DMA or claim radio
shutdown. The future owner must coordinate scheduler/radio/PCI teardown.

## Evidence and outstanding integration

Sixteen host fixtures are registered and compile, but have not run before
the required own-branch commit/build and raw publication. They exercise exact
WD/WP/address fields, management HT-Control/header boundaries, all rates, separate RPQ/DMA completion,
bounded resubmission and explicitly unknown attempt counts, actual RX-copy release, ring/tag
rollover beyond sixteen frames, faulted publication, stale/malformed reports,
frozen/regressing/late clocks, bounded shutdown and the diagnostic C2H codec.
The hardware model supplies explicit status and descriptor consumption; it
does not prove over-the-air transmission, acknowledgements or timing.

Native integration must first validate the actual completed channel token,
current policy and PHY/session identity, install this disjoint DMA before
mastering, configure CMAC and the zero-to-active scheduler transition, and
give this owner exclusive RPQ status routing. Existing receive-only permits
intentionally reject active ordinary TX and must not be bypassed or relaxed
by raw mask changes. Association, WPA2 and hardware traffic remain open.
