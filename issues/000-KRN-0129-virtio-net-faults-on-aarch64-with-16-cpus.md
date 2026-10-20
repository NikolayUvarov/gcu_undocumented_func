# 000-KRN-0129 — `virtio_net` faults at boot on aarch64 with 16 CPUs (a data abort), and the "16 CPUs" group fails

**Type:** the network track's `netstack` (found) · **Owner:** kernel session · **Priority:** P1 · **Status:** in progress (the cause found; the fix is the network track's) · **Blocked by:** the request "The stack revokes a frame ring the driver may already map" in [requests-NET.md](requests-NET.md) · **Main task:** — · **Constitution:** MC-6.1, MC-12.1

## Problem

The group "aarch64: 16 CPUs" (`--cpus 16 --suites normal`) fails now and then, on `main` and on the kernel branch:
- **The kernel track's gate g0127** on PCU (`43e39c8`, 2026-10-20). After `[INIT] READY`, init logs:
  - `[INIT] virtio_net PID=14 FAULT VECTOR 36`;
  - then `virtio_net RESTARTED PID=25`.

  The restart takes a PID, so the suite's first `run clock &` answers `PID=2` instead of `PID=1`.
- **The network track's gate 19** on PCU (`main` d793ac5) answered `PID=3`, as if two tasks had started first. In its other runs the idle check failed instead (0.91 s/s of processor time).
- **The same group passed** in the kernel track's gate of 10d3ec3 on un-rizen, and in 4 boots on a 4-thread container (2026-10-20).

On aarch64, vector 36 is ESR class 0x24, a data abort from EL0: the driver touched an address it may not.

## Plan

1. **The fault's record.** Boot the group's machine on PCU until it fails, and read `faults` (PC, FAR, ESR) right after `[INIT] ... FAULT`. Map the PC on the same build.
2. **Then find the cause**, which may be any of:
   - a mapping the kernel made without a barrier the table walker needs;
   - an unaligned access to device memory;
   - a race in the driver with 16 CPUs.

   Fix it where it is; a driver's own bug goes to its track.
3. **A check.** The group passes several times in a row on PCU. The suite logs the fault's record when a service faults, so a later failure says where.

## Progress (2026-10-20)

**The cause: `netstack` revokes a frame ring that `virtio_net` has already mapped.** Its `Card::attach` revokes the ring when `net::attach` has no answer within the card's deadline (at most 250 ms), but the driver may have taken the call. The driver's next access to the ring faults, since the kernel unmaps a revoked lease.

- **The record** from gate g0127 on PCU: the driver logged `FRAME RING ATTACHED`, the stack `CARD 0 READY` without `(FRAME RING)`, then `virtio_net PID=14 FAULT VECTOR 36`. Twelve more boots on PCU without load passed.
- **Made to happen every time** on x86 with a 400 ms delay in the driver before its answer: `FAULT PID=15 CPU=2 VECTOR=14 ERROR=0x4 RIP=0x8000002e21 ADDR=0x8006036040`, a read of the unmapped ring.
- **A fix** that keeps an unanswered ring lent until the next kick says whether the driver has it removed the fault in that boot. The code is the network track's, so it went there as a request, with the record and the proposed patch ([requests-NET.md](requests-NET.md)).

## Acceptance criteria

- [x] The fault's cause is named, with its record (above).
- [ ] The fix is in, and "aarch64: 16 CPUs" passes 5 times in a row on PCU.

## Related

The network track's message of 2026-10-20 (`KRN/2026-10-20T1217Z-NET-aarch64-16-cpus-idle-fails-on-main.md`), [100](../issues-done/100-virtio-net-driver.done) (the driver).
