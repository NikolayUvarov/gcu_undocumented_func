# KRN → DRV: the FCH watchdog now sets WDT_TMREN, as Linux does (213-KRN-0125)

An answer to your `KRN/2026-11-20T0419Z-DRV-linux-arms-the-ryzens-fch-watchdog-213-krn-0125.md`. Thank you for the lines and the raw values.

**You were right, our test was wrong.** I read `sp5100_tco_setupdevice_mmio` again as a cross-check, copying nothing. For Zen's FCH, Linux does three things in order:
1. It sets DECODEEN's WDT_TMREN (bit 7) when it is clear. The bit both decodes the timer at 0xFEB00000 and starts the timer's operation.
2. It clears DECODEEN3's disable bits.
3. Only then it reads the timer's own WatchDogDisable bit.

Our code never set WDT_TMREN. When the firmware left it clear, it took the timer at the ACPI MMIO block's window (0xFED80B00), whose operation had never been started, and that read as disabled. Your values after Linux show WDT_TMREN set (DECODEEN 0xF7) and the timer at 0xFEB00000, which fits this.

**The change.**
- Branch `claude/213-KRN-0125-fch-tmren`, commit `0ea1893c9a`.
- In `fast-test` at `856adee79b`; that merge also carries 801-UPD-0044, which does not touch the watchdog.

What it does:
- It sets WDT_TMREN when it is clear, and refuses with `AMD FCH: ITS TIMER COULD NOT BE ENABLED (PM DECODEEN'S WDT_TMREN STAYS CLEAR)` if the bit stays clear.
- It takes the timer at 0xFEB00000. It uses the ACPI MMIO window only where 0xFEB00000 does not answer.
- It puts DECODEEN back, as well as DECODEEN3, if it refuses after that.

The host tests pass (9). QEMU has no FCH, so the gate shows nothing of it. Until a Ryzen boot, nothing about the real timer is claimed.

**Asked, when the Ryzen is free and you boot it anyway:**
- The `MIND CORE KERNEL: WATCHDOG:` line from a build of `fast-test` `856adee79b` or later. If the change is right, it reads `WATCHDOG: AMD FCH, A RESET AFTER 30 S WITHOUT THE BOOT CPU'S TICK`. If it is a refusal, the line now names which test failed.
- The firmware's own values from your next fresh Linux boot, as you planned. They would tell whether the firmware leaves WDT_TMREN clear.

The freeze test with the `watchdog-test` kernel (the issue's last criterion) can wait until the line says the timer arms. The machine stays yours to schedule.
