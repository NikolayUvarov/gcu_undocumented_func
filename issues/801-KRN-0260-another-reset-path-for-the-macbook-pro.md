# 801-KRN-0260 — Another reset path for the MacBook Pro, whose firmware holds NO_REBOOT

**Type:** kernel (`kernel/src/arch/x86_64/watchdog.rs`) · **Owner:** `KRN` · **Priority:** P2 · **Status:** open (2026-11-19) · **Blocked by:** — · **Main task:** [801](801-the-bench-stays-manageable.md) · **Constitution:** MC-9.3, MC-12.3

## Problem

[801-KRN-0258](../issues-done/801-KRN-0258-a-trial-that-hangs-before-init-needs-a-watchdog.done) left this remainder (its plan's step 4).

On the MacBook Pro (7-series PCH, `mind_353abc18de2dbd0e`) the boot line says `WATCHDOG: NONE (NO_REBOOT HELD BY THE FIRMWARE)`:
- the TCO cannot reset the machine;
- that PCH has no over-clocking watchdog such as the DEXP's ([801-KRN-0259](../issues-done/801-KRN-0259-the-pchs-over-clocking-watchdog.done)).

So a trial there survives by itself a stop in the loader, a panic and a hang its tick sees. A stop of the tick waits for a person (`docs/profile`).

## Plan

1. Look for another path that resets the machine with no CPU running and no SMI that the firmware keeps:
   - the 7-series PCH's other timers;
   - the SMC;
   - a chipset feature the firmware leaves usable.

   Each is named with its source and what was read on the machine.
2. If one is found, arm it as the TCO is armed, and run the `watchdog-test` kernel's trial on the MacBook. That needs the maintainer's word; the MacBook gets no release until then.
3. If none is found, say so in the profile, with what was read, as the limit of that machine.

## Read on the MacBook (2026-11-19; read-only, no release)

1. **The TCO:**
   - the boot line of its running kernel, from 2026-11-18: `WATCHDOG: NONE (NO_REBOOT HELD BY THE FIRMWARE)`;
   - the PCH is 7-series (its EHCI controllers are 8086:1E2D and 8086:1E26);
   - in Intel's ICH and PCH documents, `NO_REBOOT` follows a strap sampled at power-on, and software cannot clear it while the strap holds it. That is a limit of the board, not something to work around.
2. **ACPI's watchdog tables:**
   - `log:acpi/` holds the machine's 22 tables, but no `WDAT`, `WDRT` or `WDDT` (`collect_file`: not-found);
   - `FACP` was read the same way as a control.
3. **Not yet looked for:**
   - an over-clocking watchdog on the 7-series PCH, which Intel's documents name for later PCHs;
   - the SMC, which has no public interface known here for a watchdog.
4. **A path the kernel could make itself:** another CPU that sees the boot CPU's tick stop and resets the machine through the reset register.
   - It would cover a stop of the boot CPU alone, as the `watchdog-test` kernel makes, if the other CPUs run by then;
   - not every CPU stopped, nor a stop before they start.
   - Not built.

## Acceptance criteria

- [ ] Each candidate path is named with what was read on the MacBook.
- [ ] Either a trial stopped before `init` on the MacBook is reset by itself, or the profile says none of the paths works there and why.

## Related

[801](801-the-bench-stays-manageable.md), [801-KRN-0258](../issues-done/801-KRN-0258-a-trial-that-hangs-before-init-needs-a-watchdog.done), [801-KRN-0259](../issues-done/801-KRN-0259-the-pchs-over-clocking-watchdog.done).
