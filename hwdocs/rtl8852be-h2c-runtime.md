# RTL8852BE serial runtime command sequence

This DRV-0032 continuation permits the existing owned packet session to retain
one normal H2C engine beyond 256 commands. Startup keeps its original refusal
to reuse sequence numbers. No firmware reset, second writer or reconstruction
of a live engine is introduced.

## Source and trust boundary

Pinned Linux `05f7e89ab9731565d8a62e3b5d1ec206485eeb0b` defines the normal
counter as `u8` in
[`core.h:4765`](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/core.h#L4765).
[`fw.c:1480–1501`](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/fw.c#L1480)
sets explicit receipt/done flags, requests receipt at least every fourth
command and advances that counter, including normal wrap. The implementation
continues using the source's BSD-3-Clause option and existing Realtek notice.

The vendor tree `68212c16b229080742280f954b3ffde173e63fcc` independently
corroborates protocol facts in
[`fwcmd.c:382,411,1008,1069`](https://github.com/HRex39/rtl8852be/blob/68212c16b229080742280f954b3ffde173e63fcc/phl/hal_g6/mac/mac_ax/fwcmd.c):
counter increment, modular receipt-distance accounting, receipt parsing, and
packet-offload done handling that requires the received state. File SHA-256 is
`206263c5cadc5f3a46490b12c25d84310bd1249225c189db17e5da15483e39a7`.
This GPL-only implementation is not copied. Its OUTSRC receipt handler returns
without updating the host `rec_seq`; it is not evidence of a global BTC delivery
barrier. Our explicit receipt flags and completion rules remain per command.

The firmware is trusted to return requested ACKs for the submitted command,
without arbitrary replay after the same wire identity is reused. A host ticket
includes a nonwrapping generation, but the wire contains only the 8-bit tag:
an ancient duplicate after a full wrap is intrinsically indistinguishable.
This change does not claim protection against malicious firmware or prove
exactly-once physical behavior. No later command's DONE acknowledges earlier
asynchronous work.

## Actual owner and completion

Only the consuming DataSession enables serial runtime on the existing engine,
after its real mode/queue/radio ownership validation, CH12 reclaim with no
pending descriptors, and command DMA idle. These reads and the final clock
check use the original activation deadline. An uncertain activation fails with
the existing retained-memory shutdown behavior.

Serial mode permits one outstanding or uncollected software command. Every
command requests receipt, plus any explicitly requested DONE. Either missing
class prevents completion; a rejected DONE also waits for receipt before its
result can be collected. The concrete Session waits for CH12 consumption and
DMA idle before a successful return or another command. Rejection is terminal
and does not establish DMA quiescence. ACK and descriptor consumption are
separate observations. A full pre-publication queue consumes
no sequence or generation; uncertain publication or timeout remains terminal.

The host counter, ticket generation, Session observation and initial-PHY
diagnostic count use `u64`. The counter refuses exhaustion before publication;
it never silently wraps or saturates. Its low eight bits form the wire tag.
Old software tickets cannot inspect or take a new result with the same tag.
No global RX-empty barrier is added: the sole reader continues processing
ACKs, BTC, PPDU, raw TX reports and bounded ordinary Wi-Fi pressure.

## Validation scope

Seven core cases compile for generation past 65535, stale software tickets,
startup handover, both receipt/DONE orders, rejected DONE, Full/exhaustion,
missing ACK classes and late previous-command ACKs. Three cumulative cases
compile for actual activation with busy command DMA, 514 commands while valid
Wi-Fi continues, and ACK completion with command DMA deliberately unconsumed
after 260 commands. Existing startup-256 tests remain unchanged in behavior.
The standalone H2C harness imports the actual RX modules now used by its shared
device model; no test is disabled. Execution follows own build and raw
publication. This is not native or hardware evidence.

The first own-branch execution of `7e60f2b6`, after raw publication
`2e267e4bc17342321920c652f8c1b9f9af1d08da`, passed all 20 H2C cases. The
complete data-mode harness passed 286 and failed both new long-command cases:
the host station model's `u8` SRAM-selector restoration counter overflowed at
`tests/rtw89_station_device.rs:43`. This did not establish a driver failure or
successful sustained traffic. The correction widens that counter and its
fault-injection ordinal to `usize`, keeping the existing exact second-restore
deadline fault. Production is unchanged; the original failures are retained
and the long cases and station validation will be repeated after publication.

The corrected `4d9f9b36` run after raw publication
`4a4fa150bc430660efe696d302041a9219c36711` passed 287 data-mode cases and
failed the remaining continuous-traffic assertion. All 514 commands completed,
but the fixture replenished Wi-Fi after every released ACK as well as data,
growing its backlog and exhausting the finite 8192-frame producer. The model
now replenishes only the last published RX record, before its release, keeping
one Wi-Fi record available without accumulating a new record per ACK. The test
retains 514 commands, the original finite producer, pressure and subsequent TX
completion assertions; every command additionally checks the actual RX-index
backlog (one pending, at most two with its receipt) and producer progress.
The second failure is retained. This correction changes no production code;
execution of the corrected model still awaits raw publication.
