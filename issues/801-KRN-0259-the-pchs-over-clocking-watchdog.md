# 801-KRN-0259 — The PCH's over-clocking watchdog: a reset no firmware SMI can keep

**Type:** kernel (`kernel/src/arch/x86_64/tco.rs`, `watchdog.rs`, `acpi.rs`) · **Owner:** `KRN` · **Priority:** P1 · **Status:** open (2026-11-19; criteria 1–3 met on the DEXP, the restart's line left) · **Blocked by:** — · **Main task:** [801](801-the-bench-stays-manageable.md) · **Constitution:** MC-9.3, MC-6.1, MC-12.1, MC-12.3

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

## The DEXP's runs (2026-11-19; MC-12.1: this machine, these boots)

The maintainer was at the machine. Releases were built from this branch and sent through Effector's `update_agent`.

1. **Release 4, as built** (`ad4555a880`):
   - taken at the second try; the first download was cut (below);
   - `updated 4, slot A confirmed` at 12:54:33 UTC;
   - its boot lines:
     ```
     MIND CORE KERNEL: WATCHDOG: THE PCH'S OVER-CLOCKING WATCHDOG AT 0x1854 (FOUND 0x2000), A RESET AFTER 30 S WITHOUT THE BOOT CPU'S TICK
     MIND CORE KERNEL: WATCHDOG: INTEL TCO OF THE ALDER LAKE-M AND -N PCH (SMBUS 8086:54A3, TCO AT 0x400), A RESET AFTER 30 S WITHOUT THE BOOT CPU'S TICK; THE FIRMWARE'S SMI AT THE FIRST EXPIRY KEPT
     MIND CORE KERNEL: WATCHDOG: SMI_EN 0x80002033, TCO1_CNT 0x1000
     ```
   - **What they show:**
     - the over-clocking watchdog was off and unlocked, as the firmware left it, with bit 13 set (`ICCSURV` on the older PCHs);
     - it was armed and fed: the machine ran on;
     - `TCO1_CNT` has `TCO_LOCK` (bit 12) set, so `TCO_EN` stays set in `SMI_EN`. That is 0258's cause, now read, not inferred.
2. **Release 5, with the `watchdog-test` kernel:**
   - taken at the second try, after its first download was cut;
   - the machine went down at 13:09:36 and was back at 13:11:03, 87 s later, with `fell back: slot A runs version 4, not 5`;
   - slot A's boot: `THE NEWER RECORD: SLOT B NOT CONFIRMED`, and the over-clocking watchdog `FOUND 0x802000`. The firmware had cleared the status bits and set scratch bit 23. An ordinary reset left 0x2000, and `FORCE_ALL` was clear, so the mark is the watchdog's expiry.

   **The reset was the watchdog's, not a hand's:**
   - the 87 s are two boots and the 30 s;
   - a reset by hand leaves no such mark.

   Criteria 2 and 3 are met.
3. **The mark:** release 4's kernel read only the status bits, so that boot did not say `IT RESTARTED THE LAST RUN`. `14a3905f64` reads the mark as a restart and clears it.

   Releases 6 (as built) and 7 (`watchdog-test`) are to show two things:
   - whether an ordinary reset keeps the mark;
   - the line after an expiry.
4. **The downloads:** five of the eight archive downloads (29 MB each) were cut after 184 to 280 s, with `/api/commands/file BODY: Transport`.
   - Each time the radio's traffic fell in the last 10 s, which is the agent's bound on a body read with no byte.
   - The radio counted no frame lost.
   - The ones taken lasted about 9 minutes.

   This is 801-NET-0088.

## Acceptance criteria

- [x] Host tests against a model of the register: off and armed, after an expiry's reset, locked and running, locked and off, silent, stuck; the feed, the longest count and the stop (`tests/tco_watchdog_host.rs`).
- [x] The DEXP's boot line names its over-clocking watchdog, armed (release 4, 2026-11-19).
- [x] On the DEXP, a trial that stops before `init` is reset by it and boots slot A by itself (release 5, 2026-11-19; 0258's criterion 2).
- [ ] The boot after such a reset says the watchdog ended the last run, and the boot after an ordinary reset does not (releases 6 and 7).

## Related

[801-KRN-0258](801-KRN-0258-a-trial-that-hangs-before-init-needs-a-watchdog.md), [801](801-the-bench-stays-manageable.md), [000-KRN-0076](000-KRN-0076-a-frozen-machine-restarts-and-starts-safely.md).
