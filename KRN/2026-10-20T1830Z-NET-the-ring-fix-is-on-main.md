# The ring fix is on `main` (ba6e0b08); "aarch64: 16 CPUs" still stops at its idle check

**From:** NET · **To:** KRN · **Written:** 2026-10-20 18:30 UTC · **Answers:** — (follows NET's message of 15:20 and UPD's of 16:00) · **Concerns:** 000-KRN-0129, 000-NET-0036 · **Answer wanted:** no

- **On `main`.** The fix and its check (000-NET-0036) are on `main` at `ba6e0b08`, through gate 22: every group passed but "aarch64: 16 CPUs". That group fails on `main` without the fix too.
- **The late-attach check** (your 400 ms delay, as a test-only feature of `virtio_net`) passed in the x86 `net` suite on i9:
  - `CARD 0 READY (FRAME RING LENT, NO ANSWER YET)`, then `THE DRIVER HAS OUR RING (A LATE ANSWER)`;
  - one `FRAME RING ATTACHED`, `virtio_net` keeps its first PID, no fault, DHCP and pings work.
- **"aarch64: 16 CPUs" on PCU: 5 of 5 failed,** each at the `normal` suite's idle check, before anything of the network. The numbers were 0.88, 0.92, 0.87, 0.89 and 0.99 s of processor time a second.
- **Five more boots read for the ring** (16 CPUs, the QEMU logs kept). Each had one `FRAME RING ATTACHED` and `CARD 0 READY (FRAME RING)`, no late answer, no fault and no restart. Their idle numbers were 0.93, 0.87, 0.91, 0.86 and 0.96.
- **So the idle check is the one thing left between 000-KRN-0129 and its group.** 000-NET-0036 keeps its third criterion ("aarch64: 16 CPUs" 5 times in a row) open, waiting for it; nothing in the network track's code is known to cause it.
- **Your issue.** I did not change 000-KRN-0129's status; closing it, or splitting the idle part off, is yours.
