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
being relabelled or silently removed. The channel remains fixed; changing it
requires a separate calibration and ownership transition. The active filter
continuation below handles scan/BSSID/local-address changes on this channel.

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

The [owned runtime BTC continuation](rtl8852be-btc-runtime.md) processes known
BT events during actual command waits and reconciles the current normal/no-link
policy in finite batches. Associated-role/scan-policy inputs are not inferred
from a raw address filter; that larger policy boundary remains explicit.
The [serial runtime H2C continuation](rtl8852be-h2c-runtime.md) preserves the
startup bound while admitting sustained commands only through the actual idle
packet owner, with per-command receipt, requested DONE and DMA completion.
Native input/grants, reset/reconnect, service dispatch and
physical under-load validation remain open under DRV-0032.

## Association boundary

[`wlanhw` 1.1](../idl/wlanhw.wit) deliberately exchanges raw MPDUs, channel,
filter and rate information. [`wlan::Wlan::apply`](../wlan/src/main.rs) forwards
those actions from [`dot11::Station`](../dot11/src/station.rs): `management`
handles authentication/association, `eapol` runs the WPA2 handshake, and `data`
and `send_frame` decrypt/encrypt CCMP in software. Keys remain with the station
and its keystore. Neither an additional key-upload interface nor hardware WPA2
offload is required by that software design.

The current [`station::prepare` and `cam`](../rtw89_wifi/src/station.rs) still
configure a NO_LINK client, disconnected JOIN and zero AID. `set_filter` changes
CAM/BSSID/address selection, not association state. In contrast, pinned Linux
[`core.c:4889–4905`](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/core.c#L4889)
updates association CCTL, JOIN(false), and CAM AID/MACID/network type during
`rtw89_core_sta_link_assoc`. The current wlanhw contract conveys neither link
state nor AID; `dot11::Station::management` does not retain the response AID.

Whether the selected fixed-rate, software-crypto raw path needs those hardware
association updates remains unconfirmed. The host packet model does not establish
real auto-ACK, protected-frame reception or WPA2 association in NO_LINK mode.
This is a validation boundary, not an established hardware blocker or a reason
to invent association from a filter update. If source or physical evidence
establishes a required link-state/AID notification, coordinate an explicit,
versioned NET interface change while preserving software key ownership and
existing radios. The native policy/endpoint KRN requests address configuration
and capability routing; they do not establish this device behavior. Actual
WPA2 association, DHCP and traffic under load remain pending.

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

Native source `9a2cc7e2` and integration `69b40829` built the complete own x86
image (2 seconds). After raw publication `554cbada`, four `native_cases::`
fixtures passed (0.05 seconds, zero failed/ignored, 242 unrelated cases filtered)
and all twelve startup fixtures passed (0.00 seconds, zero failed/ignored).
The native fixture was recompiled against the published integration before
execution; the startup binary was unchanged since compilation of `9a2cc7e2`.
Configuration: x86-64 Linux 6.6.87.2 WSL2, rustc 1.101.0-nightly c36f14571.
Logs: `/tmp/mind-native-dma-results.log` and
`/tmp/mind-native-dma-startup-results.log`. No real device was accessed.


## Active filter continuation

`DataSession::set_filter` requires no pending raw TX request and retains the
existing 1–100000 microsecond absolute runtime budget. Its private-constructor
`FilterChange` view holds the actual command owner, TX queue and counters;
station code cannot substitute a successful transport callback. The existing
raw-six-byte wlanhw address policy and zero-own-address resolution are preserved.

Before CAM mutation, old buffered and queued Wi-Fi is counted/discarded through
the sole RX owner. Actual CAM H2C completion requires the normal done ACK and
command DMA consumption. During that wait and the final bounded drain, ordinary
Wi-Fi is counted as previous-epoch data; matched ACK/BTC and packet reports keep
their owned routes. Unrelated retained control records block the transition
without being erased. Source receive-filter writes, station/selector readback,
live data/radio identity and final deadline checks precede software publication
of the new filter. Busy and invalid timeouts do not send CAM or stop the current
TX; uncertain mutations terminate with retained DMA and independent shutdown.
The empty observation retires already published RX descriptors; no hardware
airtime boundary is inferred for a frame still inside the receive engine.
This updates address selection, not association or connected BTC policy.

The station register/CAM source and licence are unchanged from
[station provenance](rtl8852be-station.md). Ten added cumulative cold cases
compile: exact CAM payload and ACK, selected BSSID and own-address delivery,
transmitter identity, banked/queued/in-flight epoch retirement, admission,
missing ACK or DMA, corrupted/applied-failed/late filter writes, selector restore
failure, retained control refusal and return to scanning. Execution is pending
this continuation's own build, commit and raw publication.


Active-filter source `96070fbe379eea198b13ff72664a68bb0ce64f65` passed all ten
new cold cases and thirteen existing packet-mode cases as part of the complete
data-mode harness after raw `4ea2d9eb93cf91c59491fc95de887ee01e081888` publication.
The own-branch complete runs passed: data mode 264, BTC Session 241, station 263,
post-MAC 241 and TX 241 (overlapping fixtures, not independent totals), with zero
failures and the same one inherited private-firmware ignore in each. Execution
was 1.07–1.21 seconds per harness on 2026-10-16; native compilation passed in
1.55 seconds. The manifest `/tmp/rtw89-active-filter-validation-96070fbe.json`
records exact source/publication, commands' results and per-harness log paths.
These remain host-model observations, not physical radio or native-service evidence.

## Owned connection orchestration

The private `connection::run` joins the production initial-PHY, channel,
nonclone calibration admission, all channel RFK, station/CAM, initial scan
filter and data-owner activation stages. It consumes the actual startup Session
and an explicitly resolved policy; it has no country, power or platform default.
Only successful completion of all stages invokes the service callback with the
actual borrowed DataSession. Same-attempt capability, board, firmware, queue
and calibration checks remain in their respective production entry points.

After the callback returns, including an error value, packet shutdown runs and
its independent radio/TX results accompany the callback value and initial-PHY
observations. The callback cannot return borrowed stage state. Activation
failures retain their recorded shutdown, and initial failures preserve their
coexistence failure detail. The outer native caller must still stop RX/WCPU,
verify PCI quiescence and retain every mapping; this helper never frees DMA.
No native policy consumer or WLAN endpoint is enabled by this private function.

Before data activation the same live Session applies the existing exact report
tail: PPDU reporting, PHY-status forwarding and RTS thresholds, following
[pinned core.c:5660-5662](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/core.c#L5660).
Its original independent cleanup is preserved and the returned observation
records the actual encoded RTS value. The single RX owner must consume/count
ordinary PPDU status separately; report payload interpretation and RSSI are not
established by this orchestration. Startup and runtime PPDU-pressure handling
is a separate integration prerequisite before exposing a native endpoint.

Four cumulative cold fixtures compile for real TX after the full chain,
callback-error shutdown, missing actual initial command completion and rejected
data activation. Their execution follows own-branch build and raw publication.


Connection source `b8515c5c` and integration `498a82dc` built the complete own
x86 image (2 seconds). After raw publication `d4e55fcc`, all four
`connection_cases::` fixtures passed (0.04 seconds, zero failed/ignored,
242 unrelated cases filtered). This run includes actual `0x02ff` RTS readback,
PHY-status forwarding and callback/activation failure boundaries. Command:
`rustc --edition=2021 --test tests/rtw89_connection_host.rs -o /tmp/mind-connection-tests`,
then `/tmp/mind-connection-tests connection_cases::`; log
`/tmp/mind-connection-results.log`, x86-64 WSL2/rustc c36f14571.
These model results do not establish a native endpoint or physical link.
