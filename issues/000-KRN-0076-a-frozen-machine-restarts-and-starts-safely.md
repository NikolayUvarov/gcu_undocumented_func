# 000-KRN-0076 — A frozen machine restarts on its own, and the next start leaves out what froze it

**Type:** kernel, bootloader, `init` · **Owner:** kernel session (`KRN`, with `PRT` for the watchdog and the bootloader's lines) · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Roadmap:** track A · **Constitution:** MC-6.1, MC-6.3, MC-12.3

## Problem

On 2026-10-11 the MacBook Pro froze twice, both times in `bcm_wifi`'s stage 2 (550-DRV-0023):

- **The first time,** the boot stopped before the shell.
- **The second time,** the boot finished. The machine stopped about 6 s later, while `wm` was drawing. The last line of the log was `STAGE 2, STEP 2`; that step's first read of the 802.11 core's clock register came 3 s later.

A ring-3 driver touched the device through its own registers, as designed. The kernel cannot stop a CPU that hangs on such a read: the bus never completes it. What the system can do is recover:

- restart the machine when it stops;
- on the next start, leave out what was running when it stopped;
- tell the user both.

Today a frozen machine stays frozen until the user holds the power button. The next start then does the same again.

## Plan (to be refined in the work)

1. **A hardware watchdog.**
   - On x86 PCs, the chipset's TCO timer. On a Mac it can be held by the firmware (`NO_REBOOT`); the kernel says so in the hardware report.
   - On aarch64 boards, the SBSA generic watchdog where the firmware describes one (ACPI GTDT, device tree).
   - The kernel arms it and feeds it from the boot CPU's tick. A machine whose CPUs stop is restarted by the chipset.
2. **A record of an unfinished run.**
   - At boot, `init` sets a mark in an EFI variable through the kernel.
   - It clears the mark after the system has run for a minute past `READY`, and at a clean restart or power-off.
   - A mark still set at the next boot means the last run stopped early.
   - On a board without EFI variables, the record goes to the log volume.
3. **A safe start.**
   - With the mark found, `init` starts only what the shell and the boot disk need: no Wi-Fi, camera, sound or speech services. It logs `[INIT] SAFE START: THE LAST RUN STOPPED AT <time>; NOT STARTED: …`, names the last line of the stopped run's log, and the shell shows it.
   - The next boot without the mark starts everything again.
   - 173-KRN-0036 (a safe start on a key) uses the same start.
4. **What is not claimed.** A device that hangs the bus still stops the machine until the watchdog fires. The watchdog's timeout bounds the outage, but it is not prevention. DMA by a device to memory it was not given is main task 174's IOMMU, and interrupt storms are 000-KRN-0077's.

## Acceptance criteria

- **QEMU x86 (q35, ICH9 TCO).** A test service that stops every CPU for good, for example with interrupts off and `hlt`, gets the machine restarted by the watchdog. The next boot is a safe start that names it, and the one after it is a normal start.
- **The MacBook Pro.** With `data/wifitest.txt` present the machine restarts by itself within the watchdog's time, and the next start leaves `bcm_wifi` out. Alternatively, the hardware report says why the watchdog cannot be armed there.

## Related

- [550-DRV-0023](../issues-done/550-DRV-0023-bcm4331-microcode-runs.done) (the freezes)
- [173-KRN-0036](173-KRN-0036-safe-start.md) (a safe start on a key)
- [174](174-full-use-of-pc-hardware.md) (the IOMMU)
- [000-KRN-0075](../issues-done/000-KRN-0075-init-waits-for-every-service-with-a-limit.done)
- [000-KRN-0077](000-KRN-0077-interrupt-storms-contained.md)
