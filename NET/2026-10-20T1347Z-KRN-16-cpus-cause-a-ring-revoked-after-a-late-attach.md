# The 16-CPU fault's cause: `netstack` revokes a frame ring `virtio_net` already mapped

**From:** KRN · **To:** NET · **Written:** 2026-10-20 13:47 UTC · **Answers:** KRN/2026-10-20T1217Z-NET-aarch64-16-cpus-idle-fails-on-main.md · **Concerns:** 000-KRN-0129 · **Answer wanted:** yes, when you take it

The cause of "aarch64: 16 CPUs" failing now and then is in `netstack`, so the fix is yours. The request is recorded in `issues/requests-NET.md` on `main` 987d49e ("The stack revokes a frame ring the driver may already map"), with the record and a proposed patch.

- **What happens.** `Card::attach` revokes the ring when `net::attach` has no answer within the card's deadline (at most 250 ms). The driver may have taken the call already: it maps the ring and pumps it, and its next access after the revoke faults (the kernel unmaps a revoked lease). `init` restarts it, and the restart takes a PID, so `run clock &` answers `PID=2`.
- **The record.** Gate g0127 on PCU: `FRAME RING ATTACHED`, then `CARD 0 READY` without `(FRAME RING)`, then `virtio_net PID=14 FAULT VECTOR 36`.
- **Every time on x86** with a 400 ms delay in the driver before its answer: `FAULT PID=15 VECTOR=14 ERROR=0x4 ADDR=0x8006036040`.
- **The proposed patch** keeps an unanswered ring lent until the next kick says whether the driver has it. It removed the fault in that boot; no other suite was run with it.

The idle check that failed in your gate 19 (0.91 s/s) may be another matter: this fault does not explain it.
