# Ready: 801-KRN-0258, the chipset's watchdog on the N100's PCH, armed early; a panic on a trial restarts

**From:** KRN · **To:** LANDING · **Concerns:** 801-KRN-0258 (advances it; its criteria are the DEXP's runs after landing)

**Ready:** `claude/801-KRN-0258-a-watchdog-for-early-trial-hangs` 1fced1fafe

It is `main` 5a858e5b35 with this branch's three commits. Also in `fast-test` (d8b36a566e).

## What it brings

- `kernel/src/arch/x86_64/tco.rs`: Intel's TCO timer behind the SMBus controller for the PCHs from Cannon Lake's on (the N100's among them), besides the LPC bridge's path; `watchdog.rs` uses it.
- The kernel arms the watchdog right after its page tables and the screen, not just before `init`.
- A panic on a trial not yet confirmed restarts the machine instead of stopping it.
- Each loader sets the firmware's watchdog (five minutes and its pauses) until `ExitBootServices`.
- Tests: `tests/tco_watchdog_host.rs`; new checks in the `watchdog` and `boot` suites. Docs: `docs/profile`, `docs/update/slots`, `docs/write-disk` (EN and RU).

## Tested, in this session's container (4 CPUs, QEMU under TCG)

- `tests/tco_watchdog_host.rs`: 10 passed.
- **x86 `watchdog`:** both checks passed, the new one included: a trial that stops for good before `init` is reset by q35's TCO, and slot A boots.
- **x86 `boot`:** 32 passed, the new one included: a panic on a trial restarts, and slot A boots.
- Builds: the x86 and aarch64 kernels (plain, `watchdog-test`, `panic-test`) and both loaders, without new warnings.

The other groups, aarch64's among them, are left to the gate.
