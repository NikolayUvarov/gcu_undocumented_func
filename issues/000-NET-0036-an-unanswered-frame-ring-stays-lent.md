# 000-NET-0036 — The stack keeps a frame ring lent until the driver says whether it holds it

**Type:** network (stack) · **Owner:** `NET` track · **Priority:** P1 · **Status:** open (built) · **Blocked by:** — · **Main task:** — (for [000-KRN-0129](000-KRN-0129-virtio-net-faults-on-aarch64-with-16-cpus.md)) · **Roadmap:** D · **Constitution:** MC-2.3, MC-6.1, MC-12.1, MC-12.2

From the kernel track's request "The stack revokes a frame ring the driver may already map (000-KRN-0129)" in requests-NET.md (2026-10-20), which this task takes from that file.

## Problem

- "aarch64: 16 CPUs" fails now and then: `virtio_net` faults at boot (vector 36, a data abort from EL0), and `init` restarts it.
- The kernel track found the cause in `netstack`'s `Card::attach`:
  - it lends a fresh ring with `net::attach` under the card's deadline, at most a 250 ms turn (550-NET-0031);
  - when the call has no answer in time, it revoked the ring;
  - but the driver may have taken the call already, or take it later from its queue. `virtio_net` maps the ring and pumps it at once, then in every pass of its loop;
  - the kernel unmaps a revoked lease in its holder, so the driver's next access faults.
- The kernel track made it happen every time on x86 with a 400 ms sleep in `virtio_net` before its answer (`FAULT ... VECTOR=14 ERROR=0x4`, a user read of a page no longer mapped).

## Plan

- **`netstack`.** A ring is revoked only when the driver said it does not hold it:
  - `attach` answered `Ok`: the ring is the card's, answered;
  - `attach` refused (`Ok(Err)`): the driver holds no mapping of it; it is revoked;
  - no answer in time (`Err`): the ring stays lent, not yet answered, and the next kick goes at once. A kick's `Ok` marks it answered (`[NETSTACK] CARD n: THE DRIVER HAS OUR RING (A LATE ANSWER)`); its `invalid` leads to a new `attach`, which revokes the old ring then, as before;
  - the boot line says `(FRAME RING)` only for an answered ring, and `(FRAME RING LENT, NO ANSWER YET)` for the other.
- **The other drivers.** `rtl8168` and `wlan` (the station's card) refuse `attach` without mapping the ring (`invalid`, or `denied` for a client that only reads): nothing to change. `netstack` revokes no other memory a driver maps; `netpolicy`'s revocations are of grants, not of memory.
- **A check** (x86): `virtio_net` built with the test-only feature `late-attach` (`scripts/x86_fixtures.sh`) answers `attach` 400 ms late, after mapping the ring. The net suite boots a raw image with it in place of the driver (`late_attach_check`).

## Acceptance criteria

- [ ] The check: the stack's boot line says the ring had no answer yet, the late answer is logged, the driver maps one ring, keeps its PID and does not fault, DHCP and `ping 10.0.2.2` work.
- [ ] The net suite passes on x86 and aarch64.
- [ ] "aarch64: 16 CPUs" passes 5 times in a row on PCU.

## Related

- [000-KRN-0129](000-KRN-0129-virtio-net-faults-on-aarch64-with-16-cpus.md): the fault, its record and the proposed patch (the kernel track).
- Issue 107 (the frame ring, `net.wit` 1.2); 550-NET-0031 (the stack's deadline per card).
