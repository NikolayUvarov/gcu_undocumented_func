# 210-APL-0005 — Reset through the watchdog, without PSCI

**Type:** porting (kernel) · **Owner:** `APL` track (open) · **Priority:** P3 · **Status:** open · **Blocked by:** [210-APL-0002](210-APL-0002-board-from-the-device-tree.md); a Mac with M1 ([issues-human](../issues-human/README.md#4-a-mac-with-apple-silicon-to-test-on)) · **Roadmap:** track H · **Constitution:** MC-12.1, MC-12.3

Part of main task [210](210-apple-silicon-native.md).

## Problem

`reboot` resets the machine through PSCI `SYSTEM_RESET`, and `reboot --off` turns it off through `SYSTEM_OFF` (issue 203). An Apple Silicon Mac has no PSCI. Asahi Linux resets through the SoC's watchdog (`apple,wdt` in the device tree) and powers off through the System Management Controller (SMC), which it reaches through RTKit mailboxes (per Asahi's documentation).

## Plan

- Reset: the watchdog from the device tree, set to fire at once.
- Power off: not in this task. `reboot --off` reports that it cannot, as on x86 today, until a task for the SMC exists.
- The profile says which of the two works (MC-12.3).
- The kernel's code is `PRT`'s directory: done with `PRT`.

## Acceptance criteria

On an M1, `reboot` restarts the Mac into its boot chain (m1n1, U-Boot); `reboot --off` reports that power off is not available, and the profile says so.

## Related

[210](210-apple-silicon-native.md), [203](../issues-done/203-aarch64-smp-and-power.done), [084](../issues-done/084-reboot.done).
