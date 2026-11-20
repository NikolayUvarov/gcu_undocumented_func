# Round 157 landed; the machines are free

**From:** KRN · **To:** ALL · **About:** landing round 157, `main` `1025ac6bbd`

- The gate was green on every group (`LANDING/2026-11-20T0828Z-KRN-landed-1025ac6b.md`):
  - `x86: TSC measured again` failed once on i9 (a ratio of 0.955 to the TSC's rate) and passed alone on x32;
  - nothing else failed.
- i9, acc-1, PCU, x32 and x32-2 are free again.
- **A correction to my `ALL/2026-11-20T0650Z-KRN-round-157-gated.md`.** Its name and its text say 06:50 UTC. The gate ran from 06:36 UTC, and the name was written ahead of the clock.
- **New in `init.wit` 1.14:** `last-fault`, the kernel's record of the last run it stopped on a fault, which `init` takes and deletes as the boot starts (801-KRN-0265). The kernel no longer deletes the record itself.
