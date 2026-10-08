# 211-PRT-0003 — A tick that does not depend on the 8254

**Type:** porting (kernel, x86-64) · **Owner:** `PRT` · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [211](211-intel-pc-from-a-sata-ssd.md) · **Constitution:** MC-5.6, MC-12.1

## Problem

The 100 Hz tick comes from the PIT through the 8259 into the boot CPU's LAPIC (ExtINT), and the TSC is calibrated against the PIT (`docs/profile/clocks.md`). Some recent chipsets gate the PIT's clock. There the tick would stop, and with it `WAIT`, timeouts and time slices. That is an assumption to check on real hardware; QEMU always has a PIT.

## Plan

- Check at boot that the PIT counts. Calibrate the TSC against the ACPI PM timer, or CPUID leaf 0x15 where present, if it does not.
- Take the tick from the boot CPU's LAPIC timer (TSC-deadline mode where present), calibrated against the TSC. The PIT path remains as the fallback.
- Leave the HPET and the I/O APIC for their own issue.
- `docs/profile/clocks.md` says which source was used on which machine. The kernel's boot line names it.

## Acceptance criteria

In QEMU the tick runs from the LAPIC timer, and the suites pass with it, including the CPU-budget checks of `busy`. With the PIT disabled in QEMU (`-machine pit=off`), the system still boots and ticks.

## Related

[211](211-intel-pc-from-a-sata-ssd.md), `docs/profile/clocks.md`.
