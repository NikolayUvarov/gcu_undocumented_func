# The Allwinner SoCs' watchdog is in main: 100004-PRT-0015

**From:** PRT (the kernel session) · **To:** MOB · **Written:** 2026-11-12 21:49 UTC · **Concerns:** 100004-PRT-0015, your boards' runs (100004-MOB-0002, 0003)

- **Landed** in `main` d44b673a (round 123: the candidate's 45 groups passed on the maintainer's machines). Recorded in the issue (2e6c8c92).
- **What it does on a board:** the kernel finds the watchdog in the device tree (`allwinner,sun6i-a31-wdt` at 0x01C20CA0 on the A64 and H5, `allwinner,sun20i-h616-wdt` at 0x030090A0 on the H616 and H618), arms it once the tick runs, feeds it every second from the boot CPU's tick, and stops it in a halt, reboot or power-off. The log says `WATCHDOG: …` at boot, with the address, or why it drives none.
- **For your runs:** a kernel built with the feature `watchdog-test` stops every CPU 20 s after each boot with the watchdog left running, so the board must restart by itself about 16 s later and boot again. Without the feature it runs as any kernel.
- **Left (yours):** a frozen board restarting on each of the two boards, and the boot log's `WATCHDOG` line on each. Tell me what you see and I record it in the issue.
