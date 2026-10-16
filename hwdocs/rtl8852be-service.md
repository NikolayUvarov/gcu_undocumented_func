# RTL8852BE private wlanhw 1.1 adapter

Issue: [213-DRV-0032](../issues/213-DRV-0032-rtl8852be-wifi.md); MC-2.3,
MC-3.3, MC-6.1, MC-12.1, MC-12.3 and Appendix B.6.

`rtw89_wifi/src/service.rs` adapts an actual, borrowed `DataSession` to the
existing generated [wlanhw 1.1](../idl/wlanhw.wit) protocol. It remains
crate-private and uncalled by the native executable. It cannot construct a
calibration permission, choose a country, resolve missing platform input,
associate with an AP or create a connected firmware role. The native
coordinator and KRN endpoint/configuration grants remain separate prerequisites.

## Request behavior

- `info` reports the actual board MAC, only the already admitted channel,
  20 MHz 2.4 GHz legacy CCK/OFDM capability, and a 2346-byte MPDU limit.
- Setting that same channel succeeds without changing hardware. Every other
  channel, including zero, is invalid. This is not multi-channel scanning.
- `set-filter` uses the actual active CAM command, done ACK, DMA completion,
  software policy and epoch retirement. Successful change also discards frames
  already serialized into the service's previous-epoch queue.
- `send` copies the generated decoder's private request into the real TX owner
  and defers its reply while subsequent IPC and RX work continue. Only the
  checked final firmware count, after RPQ and descriptor consumption agree,
  becomes `sent.attempts`; group addressing retains `acked=false`.
- `receive` uses the real retained RX descriptor and `frames::from_packet` to
  remove the FCS and write the existing eight-byte header. Signal zero means
  unmeasured. Channel and scan flag describe the admitted receive epoch.
- `attach` and `kick` return invalid, as the existing BCM adapter does: this
  slice uses `receive` and implements no shared ring.

Association, WPA2, keys and upper networking remain the station service's work.
The software filter keeps the existing raw-six-byte address semantics. A
selected BSSID does not fabricate a connected-role event.

## Bounded ownership and scheduling

Each `step` receives at most one IPC message with a 2 ms timeout, progresses
one actual RX/TX operation with a 100 ms device budget, and dispatches at most
one request. Both paths run under continuous traffic. A filter command has a
100 ms absolute device deadline. Progress retains the normal command budget
for actual BTC reconciliation; the DataSession intersects it with any pending
TX deadline. A step with a filter request can use both bounded phases, up to
200 ms plus IPC and scheduling before failure cleanup. The source-defined independent failure
cleanup still applies. These budgets do not promise a hard physical scheduling
latency or behavior of an unresponsive device.

There is one outstanding transmit call, four deferred word-only wait calls,
and four serialized frames of at most 2354 bytes. The actual DataSession inbox
remains separately bounded. Waiters reply when frames become available; a
full waiter set or its one-second lease expiry replies zero. No saved reply
is abandoned on an ordinary stop or error: pending send replies failed and
waiters reply zero. A timed-out client may make the reply fail; its local
capability and mapping are still released by the real wire implementation.

The two fixed receive slots, 9 and 10, must be reserved and empty before
construction. A deferred buffer send keeps its received memory capability
and mapping in one slot; the next request uses the other. Ordinary buffer
requests finish before that slot is reused. Constructor checks refuse an
occupied slot. The service endpoint itself must be restricted to the WLAN
station by init; no new badge or authority is invented here. The existing
wire/kernel capability, mapping and saved-reply semantics remain in the TCB.

The generated `wire::Call` owns an attached or malformed buffer capability.
The adapter does not explicitly drop `attach` a second time. Stop and Drop
finish every deferred reply and call the real independent data-mode shutdown.
`stop` returns the borrowed data owner, primary failure and shutdown result;
it never releases DMA. Outer RX/WCPU/power/PCI quiescence remains mandatory.

An unhandled retained C2H/RPQ or other control record is an explicit terminal
error with the record still held by the owner, not a Wi-Fi frame or a silently
discarded event. Known BTC event reconciliation is the separate active-owner
continuation. Its absence is not masked by this adapter.

## Validation boundary

Fifteen new fixtures execute through the cold Board, verified synthetic
firmware, capabilities, real MAC/RX/PHY/calibration, station/filter and
DataSession owners. Only kernel calls and physical hardware responses are
modeled. Tests use the production generated IDL decoder, encoder and wire
resource lifetime implementation, including deferred calls and client buffers.

The cases cover exact info and receive bytes; same-channel refusal behavior;
pending TX versus other requests and alternating receive slots; actual filter
epoch changes; bounded waiters, cancellation and save-reply exhaustion;
exactly-once received-capability release; malformed requests; continuous IPC
and RX progress; missing final TX report; retained unknown control; and Drop.
The dedicated harness is explicitly registered in `scripts/host_tests.sh` with
`--cfg rtw89_service`, so its child cases are not omitted by normal host CI.

Source `ed3808cf37d90b19c5fce5def1a6bdabb34bf390` built natively from the
crate directory in 1.70 s before publication. After raw merge
`d4e55fccc207a33edb502a14d38c7f2b95e50441` was pushed, its saved host harness
passed all 15 new service cases in 0.05 s, then the full affected harness
passed 279 cases in 1.26 s on 2026-10-16. The one inherited private-firmware
case remains explicitly ignored in the normal run; this execution did not
claim that separately fetched payload check. No test was weakened or run on
`fast-test`. The earlier build invocation from the repository root selected
the Linux target and failed on target-specific libmind output; the corrected
native crate-directory build above is the build evidence.

This slice does not establish a native endpoint, RF reception/transmission on Ryzen,
association, DHCP, ping or sustained traffic. The existing hardware acceptance
criteria remain open.

## Runtime coexistence integration regression

The follow-up merges source `f7f808b6` and exercises the actual `Service::step`
through a BT enable transition whose real H2C acknowledgment arrives after
15 ms. The Info reply must succeed, the modeled hardware must have delayed
that actual acknowledgment, and a subsequent real Send must finish with the
checked one-attempt result and no retained capability leak. This checks the
100 ms progress budget at the service boundary rather than merely calling
DataSession directly. The case is compile-only until its own build and raw
publication complete; earlier 15-case evidence above remains unchanged.

## Production coordinator composition

One follow-up cold case calls the production `connection::run`, then creates
`Service` directly in that coordinator's callback. It sends generated-wire
Info, a real active filter/CAM change and deferred TX, verifies the checked
one-attempt reply and exactly-once capability release, and stops the adapter.
The original coordinator fixture additionally checks report-tail registers,
`encoded_rts=0x02ff`, mandatory outer shutdown and retained DMA cleanup. There
is no guard reseeding between those stages. The ordinary four connection
cases stay unchanged; the composed child is included by the explicitly
registered service harness.

Source `9a000fa2da9b624533ebbdf201560d754e053b48` built natively in 3.79 s
and the saved service harness compiled before raw publication
`4a4fa150bc430660efe696d302041a9219c36711`. After that publication on
2026-10-16, the composed case passed in 0.03 s and the full harness passed
299 cases in 1.27 s, with the one inherited private-firmware case ignored.
This is evidence for that own source revision, not the later H2C changes in
the aggregate candidate or native endpoint/radio behavior.
