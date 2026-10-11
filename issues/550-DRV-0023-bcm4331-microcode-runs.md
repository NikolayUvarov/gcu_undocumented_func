# 550-DRV-0023 — `bcm_wifi`, stage 2: the BCM4331's microcode loaded and run

**Type:** driver · **Owner:** `DRV` · **Priority:** P1 · **Status:** in progress (run only on request, step by step; the core's agent reports a pending operation since stage 1b's first reset) · **Blocked by:** [550-KRN-0082](550-KRN-0082-the-bcm4331-core-stopped-before-its-bus-mastering.md) · **Main task:** [550](550-network-on-real-hardware.md), through [550-DRV-0006](550-DRV-0006-broadcom-wifi.md) · **Constitution:** MC-6.1, MC-12.1, MC-12.3

## Problem

Stages 1 and 1b ([550-DRV-0020](../issues-done/550-DRV-0020-bcm4331-read-only-probe.done), [550-DRV-0022](../issues-done/550-DRV-0022-bcm4331-core-reset-and-sprom.done)) read the chip and its SPROM, and hold the 802.11 core in reset. The core does nothing until Broadcom's microcode runs in it. Whether ours can load it, start it and hear from it is the first thing every later stage needs.

## Plan

`bcm_wifi/src/main.rs`, after stage 1b, only when window 1 holds the 802.11 core:

- **The file.** `data/firmware/b43/ucode29_mimo.fw` (for core revision 29, extracted by `scripts/proprietary.sh`): an 8-byte header (type `u`, version 1), then 32-bit big-endian words. Its size and SHA-256 are logged. Without it, the log says so and the stage stops.
- **The core out of reset,** through its wrapper: clock forced, reset released, clock no longer forced, G-mode set. Then high-throughput clock forced in the core's clock control, the PHY reset pulsed at 20 MHz, the 802.11 and PHY PLLs requested and waited for, and the MAC-PHY clock on. The PHY and radio versions are logged.
- **The microcode.** The MAC control register enables shared memory and the internal registers and holds the microcode processor at address 0. The scratch registers and the 4 KiB of shared memory are cleared, then the words written into microcode memory with auto-increment. The processor is started, and the driver waits up to 1 s for the "MAC suspended" interrupt reason, which the running microcode raises.
- **The answer.** The microcode's revision, patch level, date and time, from the first four words of shared memory, are logged.
- **Then stopped.** The processor is stopped and the core put back into reset. No DMA is set up and nothing is sent or received.

The facts come from published register descriptions (the b43 specifications) and the chip's own answers; no driver code is taken.

## Acceptance criteria

On the MacBook Pro, one boot logs the microcode's size and SHA-256, the PHY and radio versions, and either the microcode's revision and patch level, or the interrupt reason it saw instead. The core is in reset afterwards, and the machine runs on as before. This is evidence for this machine only (MC-12.1), not of Wi-Fi working.

## Progress

**2026-10-11, MacBook Pro** (`fast-test` with 550-KRN-0061's file access): the machine stopped. Stage 1b's lines end at 5.81 s; `vfs_server` mounted the boot volume at 6.96 s, so the microcode file could be read only then; the last saved line is 7.04 s, and the journal saves every 2 s. So the stop came between about 7 and 9 s, while stage 2 enabled the core. Which step did it is not known.

Since then:
- stage 2 runs only when `data/wifitest.txt` exists on the boot volume, so an ordinary boot does not run it;
- each step is logged and then waited for 3 s, longer than the journal's save period, so a stop leaves its step as the last line.

**2026-10-11, MacBook Pro, on request** (`data/wifitest.txt`): the file was read (43224 bytes, the expected SHA-256) and step 1, the core out of reset, passed. The machine stopped in step 2, the first access to a register of the core itself after the reset. In both boots that stopped, the agent's reset status read `00000002` before `bcm_wifi` did anything; in the boot that ran (stage 1b only) it read `00000000`. Stage 1b had asserted the reset without waiting for that status to clear.

Since then:
- stage 1b waits up to 300 ms for the reset status to clear before it asserts the reset, and turns the core's clock off once the core is held;
- stage 2 does not release the reset while the status is not clear, and logs the agent's registers before and after;
- stage 1 also logs ChipCommon's clock control, the PMU's registers and the PCI Express link's power state (reads only).

**2026-10-11, MacBook Pro, on request, after those changes:** the machine ran on. The reset status stayed `00000002` (the wait meant as 300 ms took 3 s: a 1 ms sleep lasts about 10 ms), and stage 2 refused to release the reset, as meant. Read as "a backplane operation still pending", that status explains the stops: an access to the core's own registers does not complete while it is set. It read `00000000` in the first two runs, before stage 1b's first reset, and `00000002` in every boot since, a forced power-off included: the card seems to keep it on standby power. Linux stops this card on such Macs with a reset pulse (pending operations given up to 300 µs, the reset asserted and released at once), not by holding the core in reset.

Since then:
- stage 1b pulses the reset the same way, and logs the status before and after;
- waits run by the clock, not by counted sleeps;
- stage 2 logs the agent's error log first, disables the core if it runs, and still does not release the reset while the status is set.

**2026-10-11, MacBook Pro, on request, with the reset pulse:** the machine ran on. The status read `00000002` before the pulse and after it; the agent's error log read all zeros; stage 2 refused as meant. Whether the status really blocks the core's registers is not known: it was set in every boot so far that touched them.

Since then, on request only, stage 2 first reads two registers of the core as the pulse left it (nothing else changed), then goes on through the enable with the status logged, not obeyed. The last step in the log tells which access stops the machine, if one does.

**2026-10-11, the cause:** after an SMC reset the status still read `00000002` before `bcm_wifi` did anything, so it is made anew at each boot. Every boot that read it was built with 550-KRN-0070, which turns the card's bus mastering off at enumeration while the core the firmware left running is still writing to memory; the two that read `00000000` were built without it. The fix belongs in the kernel ([550-KRN-0082](550-KRN-0082-the-bcm4331-core-stopped-before-its-bus-mastering.md)): stop the core with a reset pulse first, then turn bus mastering off.

## Related

[550-DRV-0006](550-DRV-0006-broadcom-wifi.md), [550-DRV-0022](../issues-done/550-DRV-0022-bcm4331-core-reset-and-sprom.done), `bcm_wifi/`, `scripts/proprietary.sh`.
