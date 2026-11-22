# 000-LNK-0050 — `usb_serial` writes in pieces of 256 bytes, so `usb_host` serves its streams between them

**Type:** driver (`usb_serial`) · **Owner:** `LNK` (done by DRV's CAM agent under the LNK owner's word, 2026-11-22, for 000-DRV-0094) · **Priority:** P2 · **Status:** in progress (2026-11-22: written, host-tested and through QEMU's suites; the Ryzen's run to come) · **Blocked by:** — · **Roadmap:** 800 · **Constitution:** MC-12.1, MC-12.2

## Problem

`usb_serial` hands the cable's converter each write whole, up to 4 KiB, as one bulk transfer, and `usb_host` waits on it.
- A CH340 at 460 800 bit/s takes up to about 90 ms for 4 KiB.
- `usb_host` serves no other request while it waits, so its other clients' streams stand still.

000-DRV-0094's run C on the Ryzen (2026-11-22, the STM-Speaker board behind the same hub as the CH340) showed it:
- all 64 dry rings of a 5-minute duplex fell inside one log read over the cable;
- each said `WAITING ON SLOT 3 ENDPOINT 4 FOR 30.7–45.4 MS`, the CH340's bulk OUT;
- the board counted 9216 capture frames overflowed and 2484 playback frames short in those six seconds.

With 0094's step 2 the IN rings stay full through such waits, but the playback (OUT) ring still ran dry 12 times: its packets come only from `audio_gw`, which cannot put while `usb_host` waits (run D).

## Plan

1. **A write goes in pieces of at most 256 bytes** (`usb_serial/src/pieces.rs`), each its own bulk transfer. A piece is 5.6 ms on the line at 460 800 bit/s, so `usb_host` serves its other clients between two, `audio_gw`'s puts among them.
2. **The throughput stays the line's:** the pieces follow one another with one request's round trip between them.

## Acceptance criteria

- [x] A host case: the pieces of every length are in order, none longer than 256 bytes, none skipped (`tests/serial_host.rs`, `a_write_goes_in_pieces_of_at_most_256_bytes_in_order`).
- [x] QEMU's `link` and `usbaudio` suites pass. (2026-11-22, LEG9, on 50f714052: `link` through QEMU's FT232, which writes through the same pieces; `usbaudio` with the audiostub check; `usb` too.)
- [ ] On the Ryzen, run D again (`audiostub duplex 300 0` with one log read in its middle): no dry playback ring in the read.
- [ ] The cable's throughput before and after, measured as a log read of a known size timed, shows the link no slower. Holds for that machine, that converter and that release (MC-12.1).

## Progress (2026-11-22)

- **Written:** 4416ff9c7. The host case passes.
- **The throughput before,** for the comparison: release 202611221044, whose `usb_serial` writes whole, 10:55 UTC, under a lease, nothing replaced.
  - Five reads of the same 137 818 bytes of the Ryzen's log over the cable (`effector485 … log from=0 max_bytes=200000`, timed on LEG9) took 4.97, 4.98, 5.00, 7.74 and 7.91 s.
  - That is 27.6 KB/s for the three steady ones; the two slow ones look like the cable tool's own overhead.
  - The same five reads go after a release with the pieces.
- **Not yet on the Ryzen:**
  - the cable's PC end (COM22) was lost at about 11:23 UTC and needs the maintainer;
  - the STM-Speaker board left the Ryzen at 11:16:42 UTC, so run D waits for it to come back.

## Related

- [000-DRV-0094](000-DRV-0094-an-isochronous-in-ring-does-not-run-dry-while-its-client-is-slow.md): its step 4.
- [000-DRV-0101](000-DRV-0101-usb-host-starts-bulk-transfers-and-answers-from-its-loop.md): the root fix in `usb_host`.
