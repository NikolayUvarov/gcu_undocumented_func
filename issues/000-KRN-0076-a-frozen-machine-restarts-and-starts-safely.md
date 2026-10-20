# 000-KRN-0076 — A frozen machine restarts on its own, and the next start leaves out what froze it

**Type:** kernel, bootloader, `init` · **Owner:** kernel session (`KRN`, with `PRT` for the watchdog and the bootloader's lines) · **Priority:** P1 · **Status:** in progress (steps 1–3 on x86 in `main` at 289e84b; the MacBook Pro's run and aarch64's watchdog left) · **Blocked by:** — · **Roadmap:** track A · **Constitution:** MC-6.1, MC-6.3, MC-12.3

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

## Progress

**2026-10-14, step 1 on x86 (the kernel track).**

- **The watchdog** (`kernel/src/arch/x86_64/watchdog.rs`). Where 00:1F.0 is an Intel LPC bridge with its ACPI I/O on and its chipset configuration in RCBA:
  - the kernel clears `NO_REBOOT` (GCS) and the TCO SMI (`SMI_EN.TCO_EN`), sets 25 ticks of 0.6 s and starts the timer with the tick;
  - the boot CPU's tick feeds it every second, so the chipset resets the machine 30 s after that tick stops (the second expiry);
  - `halt_all` (a panic, a fatal error, `HALT`) and a reboot stop it first: what a halt shows stays on the screen, and a reboot's firmware is not reset half-way.
  - On q35 OVMF leaves RCBA unmapped: the kernel maps it at Intel's address, for QEMU's ICH9 (8086:2918) only.
- **The boot line** says what was found: `WATCHDOG: INTEL TCO, A RESET AFTER 30 S WITHOUT THE BOOT CPU'S TICK`, with `; IT RESTARTED THE LAST RUN` when the second expiry's status (kept across the reset, on Intel's chipsets and in QEMU) shows it reset the last run; else `WATCHDOG: NONE (…)` with the reason: no Intel LPC bridge (AMD machines), no RCBA (newer PCHs), `NO_REBOOT` held by the firmware.
- **Left of step 1:** a CPU other than the boot CPU that stops alone is not noticed (only the boot CPU has a periodic tick); the SBSA watchdog on aarch64; the MacBook Pro run (its 7-series PCH has RCBA; whether its firmware holds `NO_REBOOT` is what the boot line will say).

**2026-10-14, steps 2 and 3.**

- **The mark** is the firmware variable `MindCoreRun` of MIND's GUID ({444E494D-4F43-4552-8076-52554E4D4152}, `RUN_MARK_*` in `common/abi.rs`):
  - `init` reads it at boot and sets it, through a firmware privilege of its own that it mints while it holds the platform privilege and drops once the mark is cleared;
  - `init` clears it a minute after `READY` (`[INIT] A MINUTE PAST READY: THE RUN'S MARK CLEARED`);
  - the kernel clears it at a reboot or power-off a task asked for (`REBOOT`), before the machine goes, and at a halt a task asked for (`HALT`, 000-KRN-0110).
  - So it is found at boot after a freeze the watchdog reset, a cut power, a panic the user turned off, or a trial boot the kernel restarted at its deadline. Without firmware variables `init` says `NO RUN MARK` and nothing is noticed: on a board without them the record would go to the log volume, which is not done.
- **The safe start:** with the mark found, `init` logs `[INIT] SAFE START: THE LAST RUN DID NOT END AS ASKED (A FREEZE, A RESET OR A CUT POWER); NOT STARTED: bcm_wifi, video_gw, audio_gw, tts` and leaves those four stopped (`svc start` starts each). The next boot without the mark starts everything.
- **The `watchdog` suite** (x86, CI group "x86: the watchdog"): the test kernel `watchdog-test` stops every CPU for good 20 s after the machine's first boot (QEMU keeps the timer's count across resets, so later boots do not). On q35:
  - the machine was reset and the next boot said so, 34 s after the stop;
  - that boot was a safe start without the four services, ran past a minute and cleared its mark;
  - after `reboot` the boot was a normal one, `audio_gw` started.
- **Left of steps 2 and 3:** the last line of the stopped run's log in the safe start's line, and the shell showing it (the tools track's part, a note in [requests-APP.md](requests-APP.md)); 173-KRN-0036's safe start on a key, which can use the same start.

## Progress (2026-10-20)

- Steps 1–3 on x86 are in `main` at 289e84b: "x86: the watchdog" passed on i9 in the gate of 10d3ec3, with every other group.
- Left: the MacBook Pro's run, and a watchdog for aarch64.

## Related

- [550-DRV-0023](../issues-done/550-DRV-0023-bcm4331-microcode-runs.done) (the freezes)
- [173-KRN-0036](173-KRN-0036-safe-start.md) (a safe start on a key)
- [174](174-full-use-of-pc-hardware.md) (the IOMMU)
- [000-KRN-0075](../issues-done/000-KRN-0075-init-waits-for-every-service-with-a-limit.done)
- [000-KRN-0077](000-KRN-0077-interrupt-storms-contained.md)
