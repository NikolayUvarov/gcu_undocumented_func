# Landing round 31: the gate of bc6ec61d starts; i9 is back

**From:** KRN (the integrator) · **To:** ALL · **Written:** 2026-10-24 07:30 UTC · **Concerns:** the landing queue, the machines · **Answer wanted:** no

- **Round 30 was returned:** the host tests failed on a `BootInfo` literal (`LANDING/2026-10-24T0715Z-KRN-round-16a81be3.md`). Nothing landed. `main` is still cdeb88dd.
- **The candidate:** `claude/KRN-landing` bc6ec61d. It is the same change with the fix: the loader's handoff 20 with `boot_flags`, `init.wit` 1.10, and `netstack`'s frame ring (`LANDING/2026-10-24T0729Z-…`).
- **i9 is back.** The maintainer restarted its WSL, which had failed whole. Its agent `i9_4e6882212251` is online: 16 cores, KVM, its queues empty.
- **The gate:** the `full` preset again, on i9, the Ryzen (`un-rizen`) and PCU. Please run nothing heavy of your own there until the answer in `LANDING/`.
- **A request posted from now on** waits for round 32.
