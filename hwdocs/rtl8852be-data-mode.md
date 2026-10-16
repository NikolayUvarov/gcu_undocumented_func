# RTL8852BE owned initial data mode

This private DRV-0032 prerequisite connects the actual source-profile channel
calibration, station/CAM and retained packet queues. It does not expose a native
WLAN endpoint or establish physical traffic, continuous coexistence or recovery.

## Source and selected queues

The source is Linux `05f7e89ab9731565d8a62e3b5d1ec206485eeb0b`, using the
BSD-3-Clause option recorded in THIRD_PARTY.md. Queue enable follows
[`pci.c:249–264,3237–3241`](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/pci.c).
The private raw TX engine owns CH0–3 and CH8 (management), so activation clears
only `0x00090f00` (these five channels plus write-pointer release). CH9 remains
stopped; command/RX/PCIe I/O remain in their already validated running state.
The complete checked STOP mask is `0x001f0f03`, with value `0x00020000`.

[`reg.h:2258–2273`](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/reg.h)
assigns CTN BE/BK/VI/VO0 bits 0–3 and MGQ bit 8: `0x010f`.
Firmware-ready scheduler programming follows
[`mac.c:3168–3210`](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/mac.c)
through the existing shared register-mailbox implementation. It checks the
correlated TX_PAUSE response and actual CTN register; no independent mailbox
sequence or direct CTN write is introduced. Host interrupt masks remain zero.

## Ownership and transition

`prepare` validates the actual PHY, Board, immutable tuned policy, completed
channel RFK, Initialized owner, configured station filter and installed TX
queue. `PreparedActivation` consumes the station token and binds the actual
nonwrapping command/TX identities and TX physical region. `activate` consumes
the startup Session and retains the exclusive TX queue borrow. Failure returns
the retained owner alongside its cause; no path frees DMA.

Startup ReceivePermit stays strict. A separate sealed Layout/DataPermit checks
the three actual command/RX/TX identities, ring bases/counts/indices, MAC and
packet mode. DataSession exposes neither startup Session nor PhyIo, including
through Drop; forgetting the wrapper cannot recover an active startup owner.
Radio reset/ZCDC/AFC/PHYREG and BB plus both analog/direct RF channel fields are
checked before activation and before runtime work. Ordinary TX busy is allowed
in data mode; it never authorizes a startup/calibration operation.

One absolute one-second activation deadline includes initial RX-epoch drain,
station and radio readback, DMA enable, mailbox response, queue start and final
ownership checks. Runtime calls accept 1–100000 microseconds, include live
validation and final clock checks, and cannot extend a pending raw TX request's
original deadline. The finite MMIO model is not a physical timing guarantee.

Before first delivery, buffered old Wi-Fi frames are counted/discarded and both
RX rings are drained until an empty observation, with at most 513 poll pairs
inside the same deadline. Retained non-Wi-Fi records block exposure rather than
being relabelled or silently removed. The initial filter/channel is immutable;
runtime filter changes and channel switching need their own epoch transition.

## One RX reader and pressure

The same Session processes RX during normal H2C waits. Matched ACKs and BTC events
keep their existing routes. The typed data handler consumes actual RPQ and final
MAC TX_REPORT records through Queue::ingest; Queue::advance never reads RX.
Wi-Fi records pass the frame integrity/header/rate/FCS decoder and the configured
address predicate. Accepted records use the caller-owned four-slot inbox.
Ordinary Wi-Fi overflow drops the incoming frame and increments `pressure`;
`filtered`, `invalid` and `previous_epoch` are separate saturating counters.
These drops do not abort a command or fabricate TX completion. Other control
records retain explicit terminal overflow, including the fifth consumed copy.
Handler errors preserve the current descriptor and bytes before scratch reuse.

Raw TX validates the configured transmitter address. Busy, invalid length/rate,
invalid timeout/body and a foreign transmitter are admission errors without
shutdown or descriptor publication. Hardware, ownership, unknown completion,
clock and deadline failures poison the owner and independently attempt radio
off and owned packet-DMA stop. Shutdown is attempted once and its result cached;
DMA remains retained until the caller separately verifies PCI quiescence.
Dropping the wrapper only poisons software state; explicit shutdown is mandatory.

## Remaining runtime dependencies

BTC events are still retained for the existing policy owner; this slice does
not implement data-mode event reconciliation or the subsequent common action.
Without that continuation, its bounded event storage can fill and stop the
private session. Native endpoint exposure must wait for that owned processing
path. The normal H2C engine still refuses command 257 instead of wrapping without
a receive-order contract. This is an initial-session prerequisite, not a complete
continuous service. Native input/grants, reset/reconnect, service dispatch and
physical under-load validation remain open under DRV-0032.

## Validation

Thirteen new cold continuation cases and the strict original ReceivePermit
regression compile. The cumulative model executes actual production firmware,
MAC/RX, initial PHY, checked calibration policy, all channel RFK, station/CAM and
data-mode helpers on one device; it does not reseed guards between stages.
It models immediate hardware consumption and does not measure RF behavior.
Cases exercise ignored/applied/late enable, mailbox failures, radio tampering,
old RX epochs, TA/Busy/invalid admission, actual three-part TX completion,
terminal control retention and 12 accepted Wi-Fi frames plus three foreign and
one invalid frame during command/TX-report progress. Execution follows own build
and raw publication; no test result is claimed here yet.


The first published run of source `12a98125` on its own branch (raw
`ea703b492d04d99ae5e52d00bb0865335a5182f7`) passed 12/13 focused cases
(2026-10-16, 0.17 s). The missing-final-C2H fixture incorrectly assumed that
forty immediate model polls consumed ten milliseconds; it observed no false
completion but failed to reach the requested expiry. The correction advances
the modeled monotonic clock explicitly beyond the original request deadline
and retains the timeout/no-later-success assertions. Production code is
unchanged; the failing log remains `/tmp/rtw89-data-mode-focused-12a98125.log`.


Corrected source `974088b104d11671cdcaadff8707a38f3fbf009e` passed all
13 focused cold cases (0.17 s, no failures/ignores) on its own branch after
raw `33fa88bff00df750b3265d4dacd9be72716f93a3` publication. All affected
complete harnesses also passed (2026-10-16; overlapping tests, not a sum of
independent cases): data mode 252, BTC init 239, BTC Session 239, calibration
policy 251, channel RFK 246, DPK 239, initial PHY 244, IQK 239, PHY 344,
pipeline MAC 249, post-MAC 239, station 261 and TX 239. Each complete harness
retains the same existing ignored private-firmware hash case; no new skip or
ignore was introduced. Each was recompiled at the tested source; execution
was 1.20–2.59 seconds per harness. The original ReceivePermit rejection is
included. The JSON result manifest is
`/tmp/rtw89-data-mode-validation-974088b1.json`, with per-harness log paths.
These are host-model results; the issue's physical acceptance remains open.

## Native retained packet queues

`native_dma::Queues` now constructs command, RX and packet-TX owners from one
retained 8 MiB coherent allocation. Command keeps its first 128 KiB; RX owns
exactly its 5,902,336 bytes; TX begins at offset 6,033,408 and receives only the
remaining disjoint bytes. The TX engine uses 135,168 bytes there. All boundaries
are page aligned and the entire grant must fit below 4 GiB. No alias to the
allocation is used after these owners are constructed. Partial construction
failure cannot release the `ManuallyDrop` allocation or enable mastering.

The native sequence installs TX after RX and before kernel mastering. All five
packet queues and WP remain stopped through command-only activation and the
initial PHY diagnostic. No calibration policy or working endpoint is fabricated.
Cleanup independently attempts packet TX, RX, radio and command/WCPU stop,
then retains the existing checked MEM-only / power-off / quiescence sequence.

Four new fixtures compile for actual cumulative GEN1/GEN2 setup and installed
ring addresses, untouched command padding/TX tail, invalid and mastering grants,
and early installation refusal. The twelve orchestration cases now include
failure at the added sixteenth stage. Execution is recorded after publication.
