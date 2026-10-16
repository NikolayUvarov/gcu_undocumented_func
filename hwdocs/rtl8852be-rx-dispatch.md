# RTL8852BE retained RX dispatch prerequisite

Issue: [213-DRV-0032](../issues/213-DRV-0032-rtl8852be-wifi.md).
This changes library ownership, not native activation or the service ABI.
The single `btc_session::Session` retains exclusive access to both RX rings,
including during command receipt/DONE waits. A private TX engine no longer
polls the hardware receive rings in production.

## Bounded caller-owned storage

`Session::new_with_inbox` borrows an `Inbox` for its entire lifetime. Its four
slots each hold at most `rx::BUFFER_BYTES` (11,498) bytes and descriptor/channel
metadata. The caller allocates the bank outside the small native stack. The
Session contains only a borrowed reference and overflow metadata; no packet
allocation occurs on RX. A bank binds once to an actual command queue session,
and cannot be reattached even after its records have been consumed.

Matched normal H2C ACKs still feed the existing engine; BTC events still feed
the separate ordered policy queue. Wi-Fi data, RPQ, other C2H and unrelated
or unexpected ACKs are copied into the runtime inbox before scratch reuse.
Metadata retains CRC/ICV/decryption flags; receiving a frame does not assert
that it is valid for delivery. Malformed descriptors/C2H and C2H on RPQ remain
terminal errors. Matched duplicate ACKs remain the ACK engine's responsibility.

Records are ordered by dispatcher observation: at most one RXQ record followed
by one RPQ record per polling round, not an invented cross-channel hardware
timestamp order. `packet()` borrows the oldest record; its owner processes it
before `consume_packet()`. All bytes remain available after DMA buffer release
and scratch reuse. `poll_packets` reads at most one descriptor per channel,
validates live ownership and checks one absolute deadline including final work.
Command wait loops retain their existing finite-call/frozen-clock bounds.

If the four-slot bank fills, the next record produces `InboxOverflow`. The
session becomes terminal and does not poll, republish or reuse scratch again.
The first four records remain in the bank; the fifth is inspectable through
`overflow_packet()` in the now-frozen scratch buffer. This is explicit failure,
not a claim that an arbitrarily long traffic burst can be buffered. Raw register
cleanup remains available and DMA storage remains retained until verified
external shutdown.

The RX engine records validated full-copy metadata for each invocation before
attempting release, clearing it before every new invocation. A late release
or caller-clock failure can therefore expose the actual fresh packet through
`terminal_packet()` without parsing stale scratch. This diagnostic may also
be present in the bank and does not assert successful descriptor release.
Partial copies, empty polls and errors before a full parse produce no such
record. All these error paths remain terminal.

The existing `Session::new` is deliberately unchanged for the current pre-data
startup diagnostic: it counts and discards packets outside ACK/BTC routes.
It is not admissible for a runtime RX/TX service; `retains_packets()` distinguishes
that mode. The native diagnostic does not yet allocate a runtime bank or activate
an endpoint. Existing receive-only permits continue to reject ordinary TX.

## TX handoff

The single RX owner supplies copied records to `tx::Queue::ingest` together
with fresh monotonic time. It returns unrelated records to their handler and
never reads registers, releases RX ownership or publishes another descriptor.
`advance` separately checks live TX state and actual DMA consumption; completion
still needs consistent RPQ and final firmware reports. The caller must not
consume an inbox record until its actual handler accepts it. Ingest and advance
share the same request deadline and finite-call budget. No exact on-air attempt
count or native readiness is inferred from this routing layer.

Typed active-mode admission, actual station/filter setup and a native loop that
drains retained records remain separate integration work. The H2C engine's
existing startup sequence-lifetime limit remains unchanged.

## Validation scope

Eight new Session fixtures cover command-wait interleaving of Wi-Fi/RPQ/other
C2H/foreign ACK/BTC, matched ACK separation, full-size copy ownership and bank
wrap, explicit overflow with the fifth record retained, wrong-channel C2H,
frozen-clock timeout retaining preceding data, late release/post-return deadlines, and rejection of stale scratch metadata. Three new TX fixtures use
copied records to exercise completion without any TX receive-index access,
return unrelated data, reject corrupt RPQ and share a finite frozen-clock
budget between ingest and progress. Two new RX fixtures distinguish a validated copy from successful release and reject partial/previous-copy metadata. Existing Session/TX/RX and cumulative
fixtures remain applicable. Compilation and own release build precede raw
publication; test execution evidence will be added after that publication.
