# 000-NET-0015 — A network card's client that only reads it

**Type:** network (driver, interface) · **Owner:** `NET` track (worked by the storage session at the maintainer's request) · **Priority:** P3 · **Status:** open (built; its check waits for `KRN`) · **Blocked by:** [requests-KRN.md](requests-KRN.md) (a holder of the badged client) · **Main task:** — (for the tools track's [000-APP-0049](000-APP-0049-network-traffic-on-wms-background.md)) · **Roadmap:** track D, G · **Constitution:** MC-3.11, MC-12.4, Appendix B.6

From the tools track's request in requests-NET.md ("A read-only client of a card's counters, for `wm`'s background", 2026-10-10).

## Problem

`wm` can show the network traffic on its desktop, but a card's counters (`net.wit` `counters`) are reached only through a full client of the driver, which can also send and receive frames and lend it a ring. Lending that client to `wm` would give `wm` the card itself.

## Plan

- **`idl/net.wit` 1.3:** a client with bit 0 of its badge set (`BADGE_COUNTERS` in `mind::network`) may call `info`, `counters` and `offloads`.
  - Its calls with a result answer `denied`, a new last case of `error`.
  - `wait` is rejected.
  - A ring it lends is dropped unused.
- **`virtio_net`** enforces it. A later driver of `net.wit` (the USB Ethernet driver, the station) does the same.
- **Who mints and lends it** is not the network track's:
  - a request flag, and the right to mint, from `KRN` (requests-KRN.md, "A read-only client of a network card for a program");
  - the shell's lending and `wm`'s use from `APP` (000-APP-0049).

## Acceptance criteria

- `virtio_net` answers a client with the badge as above. It is checked in QEMU through a client with the badge: its counters are read, and `send`, `receive` and `attach` are refused.
- A program holding the client through the shell is checked in 000-APP-0049, once `KRN` has given the flag.

## Progress (2026-10-12)

- **Done:**
  - `idl/net.wit` 1.3 and `mind::network::BADGE_COUNTERS`;
  - `virtio_net` refusing a reader's other calls (a19471a), built for x86.
- **Not tested yet:** nothing in the system holds a badged client. `init` lends the shell an unbadged copy without `CAP_BADGE`, and the network track does not change `init` (requests-KRN.md, "A read-only client of a network card for a program"). The QEMU check comes with that request, or with 000-APP-0049's lending, whichever gives a program the client first.
