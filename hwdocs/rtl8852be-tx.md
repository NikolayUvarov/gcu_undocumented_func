# RTL8852B raw MPDU transmit prerequisite

Issue: [213-DRV-0032](../issues/213-DRV-0032-rtl8852be-wifi.md).
This private library moves management and ordinary/QoS data MPDUs through
band-zero ACH0..3 and CH8. It is not yet a `wlanhw` service. Its private start transition is owned by the checked DataSession after actual
channel RFK/policy, station CMAC configuration, live session ownership and
ordinary scheduler activation. Native service exposure remains separate.
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
count in word 5. The engine requests it with SPE_RPT and SW_DEFINE, preserving its raw
counter separately from descriptor completion and on-air attempts. Live
checks require PTCLRPT_FULL_HDL.SPE_RPT_PATH=WLCPU, established by the actual
runtime-MAC stage; missing forwarding refuses descriptor publication.

Each WD selects DATA_TXCNT_LMT=1, a fixed legacy rate with DISDATAFB, no
aggregation and no RTS/HW_RTS. The upstream author/maintainer discussion
explicitly defines limit one as one transmission attempt and a returned
DATA_TX_CNT of one. The pinned RTL8852B uses that descriptor field and original
C2H count layout. See the [primary-source chain and scope](rtl8852be-tx-attempts.md).
This uses explicitly requested firmware reports on PCIe in addition to its
own RPQ and descriptor completion; USB transport completion is not substituted.

The engine advances only after matching RPQ status, PCI read-index consumption
and a consistent final C2H report have all arrived. Final DONE with a zero count,
or any report exceeding the selected limit, poisons the queue immediately.
Checked final counters are accumulated and returned as `AttemptCount::Known(n)`;
raw counters and descriptor counts remain separately inspectable. This resolves
the `wlanhw` 1.1 exact-attempt semantic prerequisite without an IDL change.
It does not by itself expose the service or establish physical transmission.

DONE ends the request. A group address always returns `acked=false` and is
never resubmitted. RETRY_LIMIT permits another single-attempt descriptor up
to the caller's explicit 1..15 bound, preserving sequence and setting the MPDU
Retry bit. Interim reports do not increment the result. LIFE_TIME/MACID_DROP
retain raw status and the count of earlier completed descriptors, then require
shutdown without inventing attempts for the dropped descriptor. DMA-only or
C2H-only completion never returns a sent result. Firmware and hardware remain
part of the TCB for the meaning and truthfulness of their reports.

## Ownership, limits and shutdown

The caller supplies 135,168 disjoint coherent WB bytes below 4 GiB on x86-64,
with strongly ordered UC MMIO and no aliases. Five 32-entry TXBD rings occupy
the first page; 32 separate 4 KiB slots hold each WD and payload. Only one
logical MPDU is outstanding. This is deliberate for the synchronous `send`
contract; there is no per-packet heap allocation. The single `Session` RX
owner retains copied records while handling command ACKs. `Queue::ingest`
accepts a retained channel/descriptor/byte record and fresh monotonic time;
`Queue::advance` revalidates TX registers and reclaims descriptor consumption.
Neither production entry reads or releases RXQ/RPQ. Unrelated packets are
returned for their own handler, and ingest alone cannot publish or complete
an MPDU. The old polling adapters exist only in host fixtures. Ingestion and
progress share the request's absolute and finite-call budgets. See the
[retained RX ownership contract](rtl8852be-rx-dispatch.md).

A payload is immutable until its actual RPQ release, descriptor consumption
and final correlated firmware report are all observed. The RPQ and descriptor
requirements follow `pci.c::rtw89_pci_release_txwd_skb` and
`rtw89_pci_reclaim_txbd`. The source's extra eight-packet retention applies to
FWCMD, not these RPQ-released payloads. Release/Acquire fences constrain CPU
ordering; x86 coherent WB/UC ordering supplies the device-visible ordering.
The allocation is retained even after errors, stop or object drop; only
external verified PCI quiescence permits its owner to free it.

WP tags rotate at 15 bits, firmware tags at four bits and descriptor indices
at 32 only after earlier ownership is fully resolved. Final firmware report
selection follows `mac.c::rtw89_mac_c2h_tx_rpt`: DONE or the configured count
reached; earlier unsuccessful reports do not release ownership. Raw counters
cannot regress within a descriptor; final RPQ/C2H statuses must agree. The
raw counts of completed descriptors are accumulated in a checked `u16`.
Recent exact duplicates are harmless; malformed, contradictory or foreign
reports poison the session. A timeout or uncertain doorbell never permits
later reuse or restart, including if a late report subsequently arrives.

The explicit firmware/device TCB contract is that an ordered terminal C2H
ends that descriptor's report stream and matching RPQ releases its DMA
payload. The upstream `mac.h::rtw89_tx_rpt_init/rtw89_tx_rpt_skb_add` cycles the
same four-bit space; this engine is stricter because it never overwrites an
unresolved tag. It supports ordinary continued operation beyond sixteen
packets. Arbitrary replay of an already terminal report after a complete tag
cycle is outside this protocol contract, rather than asserted impossible by
synthetic tests or used as an artificial sixteen-packet lifetime limit.

Each request has an absolute 1..1,000,000 microsecond caller budget, checked
before publication and after final completion. Clock regression fails closed.
Each poll has finite work, and at most `timeout_us` polls are admitted even
if the clock freezes. All counters check overflow. The independent stop sets
only owned TX/WP stop bits and checks busy state under fresh bounded budgets;
it does not disable CH12 or RX, restore calibration, free DMA or claim radio
shutdown. The future owner must coordinate scheduler/radio/PCI teardown.

## Evidence and outstanding integration

Sixteen host fixtures are registered and passed on source `97195003`, after
raw publication `6b29b215` (2026-10-16). Own native release build passed in
1.35 s without warnings. TX: 16 passed, 0 failed, 0 ignored, 0.18 s; affected
RX: 18 passed, 0 failed, 0 ignored, 0.11 s. Configuration: x86-64 Linux
6.6.87.2-microsoft-standard-WSL2, rustc 1.101.0-nightly
(c36f14571 2026-10-01), own branch only. They exercise exact
WD/WP/address fields, management HT-Control/header boundaries, all rates, separate RPQ/DMA completion,
bounded resubmission and explicitly unknown attempt counts, actual RX-copy release, ring/tag
rollover beyond sixteen frames, faulted publication, stale/malformed reports,
frozen/regressing/late clocks, bounded shutdown and the diagnostic C2H codec.
The hardware model supplies explicit status and descriptor consumption; it
does not prove over-the-air transmission, acknowledgements or timing.

Native integration must first validate the actual completed channel token,
current policy and PHY/session identity, install this disjoint DMA before
mastering, configure CMAC and the zero-to-active scheduler transition, and
use the caller-owned Session inbox and route each retained RPQ/final-C2H
record to the actual TX owner. The Session remains the only `&mut Rings`
reader, including during H2C waits. A typed transition to active data mode
is implemented by the private DataSession; concrete native dispatch integration
remains necessary. Existing receive-only permits
intentionally reject active ordinary TX and must not be bypassed or relaxed
by raw mask changes. Association, WPA2 and hardware traffic remain open.

### Final-C2H extension evidence

Twenty-four combined TX fixtures include all six orderings of
DMA/RPQ/final-C2H completion, intermediate reports, raw counter versus
completed-descriptor count, terminal missing/late reports, contradictory
statuses/tags, shared frozen-clock bounds and preservation of other RXQ
traffic. Existing 320-packet and 32,770-packet rotations now use actual modeled
C2H headers and repeatedly reuse four-bit tags. The model emits reports only
after reading a published descriptor, and overwrites released RX bytes.
On source `709721d6`, after raw publication `7ec8fbee` (2026-10-16), all
24 TX fixtures passed in 0.28 s and the unchanged 18 RX fixtures passed in
0.12 s, with zero failures/ignores. Own release build passed in 1.35 s without
warnings. Host/compiler configuration is the same as the baseline above;
execution occurred only on the own branch. No physical transmission or exact
on-air attempt interpretation is established by this host model.

### Source-defined attempt accounting follow-up

The [attempt-accounting follow-up](rtl8852be-tx-attempts.md) supersedes the
baseline Unknown result and its deliberately unresolved interpretation above.
Its build/publication/test evidence is recorded separately; the earlier
configuration results are not carried forward as evidence of this change.
