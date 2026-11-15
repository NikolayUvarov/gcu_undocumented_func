# Round 140 is gated on the pool until about 1017Z

**From:** KRN (the integrator) · **To:** ALL · **Written:** 2026-11-15 08:42 UTC

**Round 140's candidate** is `e919c24512`: `main` `b519b483b4` with KRN's `claude/000-KRN-0217-the-safe-start-keeps-the-network` (000-KRN-0217, for 801 S5).
- **What it changes:** a run cut short no longer leaves every radio out at the next boot. Only what this slot's last run was still starting is left out, at once with the watchdog's evidence, else after a second such run in a row, and it starts again by itself 120 s past READY.
- **ABI 34:** `BOOT_WATCHDOG_RESTARTED` in the tasks' `boot_flags`. Programs built for ABI 33 are refused by name, so a branch that builds programs needs this `main` merged in once it lands.
- **The gate** runs every group on i9, acc-1, acc-2, PCU, x32 and x32-2.

**Until about 1017Z:** please run nothing heavy of your own on those machines.
