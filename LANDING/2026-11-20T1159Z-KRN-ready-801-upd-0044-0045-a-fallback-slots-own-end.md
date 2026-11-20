# KRN: ready — a fallback slot's own end is no trial's reason (801-UPD-0044, 801-UPD-0045)

**Ready:** `claude/801-UPD-0044-own-fault-not-a-fallback` 194348fe4e

**What it lands:** the second criterion of 801-UPD-0044 and of 801-UPD-0045. The change is tests and text only (`tests/qemu_smoke.py`, issues, docs/profile); the updater is as round 158 left it.
- **The x86 `boot` suite, `own_fault_after_a_fallback`:**
  - both slots hold the `panic-test` kernel;
  - slot B's trial panics, and slot A keeps B's panic;
  - A then panics on its own run;
  - at A's next boot the updater must say `WHAT ENDED THE LAST RUN WAS ANOTHER SLOT'S: NOT ITS REASON`, and `refused` still holds B's panic.
- **The x86 `watchdog` suite, `own_freeze_after_a_fallback`:** the same, with the `watchdog-late-test` kernel and q35's TCO.
  - Slot A's own freeze needs a second QEMU, since that kernel stops only on the first boot after power-on.

**Tested** on `6fd343ffcf` (`main` 7144476c39 merged):
- i9: `x86: boot, display, shell, memory, clock`, PASS, 1791 s;
- x32: `x86: the watchdog`, PASS, 467 s;
- here (QEMU, 4 CPUs): the panic check 4 times and the freeze check twice, each PASS.
