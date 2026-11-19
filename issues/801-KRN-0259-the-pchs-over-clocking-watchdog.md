# 801-KRN-0259 — The PCH's over-clocking watchdog: a reset no firmware SMI can keep

**Type:** kernel (`kernel/src/arch/x86_64/tco.rs`, `watchdog.rs`, `acpi.rs`) · **Owner:** `KRN` · **Priority:** P1 · **Status:** open (2026-11-19) · **Blocked by:** — · **Main task:** [801](801-the-bench-stays-manageable.md) · **Constitution:** MC-9.3, MC-6.1, MC-12.1, MC-12.3

## Problem

On the DEXP (Intel N100, Alder Lake-N PCH) the TCO timer of [801-KRN-0258](801-KRN-0258-a-trial-that-hangs-before-init-needs-a-watchdog.md) is armed, but a trial stopped for good before `init` was not reset in 20 minutes (0258, "The DEXP's runs"):
- its boot line says `THE FIRMWARE'S SMI AT THE FIRST EXPIRY KEPT`: `TCO_EN` stays set in `SMI_EN`;
- so the first expiry goes to the firmware's SMI handler, which may reload or stop the timer.

The same PCHs have a second timer, the over-clocking watchdog, `OC_WDT_CTL` at the PM base + 0x54. Intel's SoC I/O registers describe it:
- `OC_WDT_EN` (bit 14): if the count expires before a reload (`OC_WDT_RLD`, bit 31) or a clear of this bit, a status bit is set and **a global reset** follows; no SMI is named;
- `OC_WDT_TOV` (bits 9:0): seconds, less one, up to 1024;
- `OC_WDT_CTL_LCK` (bit 12): freezes the register until the core well loses power;
- `OC_WDT_FORCE_ALL` (bit 15): set, any global reset counts as an expiry;
- the two status bits (25, 24) outlive the reset and are cleared by a 1 or by RSMRST#;
- bits 23:16 are scratch for software.

On the DEXP its DSDT names it: device `CWDT`, `_HID INTC1099`, `_STA` 0x0F, its register at 0x1854 (PM base 0x1800 in the FADT). Linux's `intel_oc_wdt` drives the same device.

## Plan

1. **`tco.rs`:** `arm_oc` arms it at 30 s, the TCO's reset time:
   - the status found is cleared once and named as a restart;
   - `FORCE_ALL` cleared, the scratch bits and the rest kept as found;
   - a register locked by the firmware is fed at its own timeout if it runs, and refused if it is off.

   Also `feed_oc`, `stop_oc` and `long_oc` (the longest count, 1024 s).
2. **`watchdog.rs`:**
   - `early` arms it beside the TCO, once the FADT is read, before the page tables, on the PCHs listed by their SMBus controller;
   - the tick feeds it, and a halt the kernel chose stops it;
   - its boot line names the port, the register as found, the timeout, a lock and a restart.

   When the TCO's SMI is kept, a second line gives `SMI_EN` and `TCO1_CNT` as read, for the cause in 0258.
3. **`main.rs`:** `watchdog::early` after `acpi::init`, since the PM base is the FADT's.
4. **`acpi.rs`:** `reset` gives it its longest count first.
   - **The reason:** these PCHs do not say whether `OC_WDT_EN` survives a host reset. If it does and the firmware leaves it, 30 s may not cover the firmware and the loader before the next kernel feeds it.
   - **The cost:** a reset that does not come still ends in one, after 17 minutes. `reboot` stops it, as it stops the TCO.
5. **The DEXP:**
   - a release as built, to read its boot line;
   - then one with the `watchdog-test` kernel, whose trial stops for good before `init`.

## Acceptance criteria

- [x] Host tests against a model of the register: off and armed, after an expiry's reset, locked and running, locked and off, silent, stuck; the feed, the longest count and the stop (`tests/tco_watchdog_host.rs`).
- [ ] The DEXP's boot line names its over-clocking watchdog, armed.
- [ ] On the DEXP, a trial that stops before `init` is reset by it and boots slot A by itself (0258's criterion 2).

## Related

[801-KRN-0258](801-KRN-0258-a-trial-that-hangs-before-init-needs-a-watchdog.md), [801](801-the-bench-stays-manageable.md), [000-KRN-0076](000-KRN-0076-a-frozen-machine-restarts-and-starts-safely.md).
