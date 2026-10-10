# 550-DRV-0023 — `bcm_wifi`, stage 2: the BCM4331's microcode loaded and run

**Type:** driver · **Owner:** `DRV` · **Priority:** P1 · **Status:** in progress (written; skipped until `init` gives the file access) · **Blocked by:** a request in [requests-KRN.md](requests-KRN.md) ("`bcm_wifi` reads its microcode") · **Main task:** [550](550-network-on-real-hardware.md), through [550-DRV-0006](550-DRV-0006-broadcom-wifi.md) · **Constitution:** MC-6.1, MC-12.1, MC-12.3

## Problem

Stages 1 and 1b ([550-DRV-0020](../issues-done/550-DRV-0020-bcm4331-read-only-probe.done), [550-DRV-0022](550-DRV-0022-bcm4331-core-reset-and-sprom.md)) read the chip and its SPROM, and hold the 802.11 core in reset. The core does nothing until Broadcom's microcode runs in it. Whether ours can load it, start it and hear from it is the first thing every later stage needs.

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

## Related

[550-DRV-0006](550-DRV-0006-broadcom-wifi.md), [550-DRV-0022](550-DRV-0022-bcm4331-core-reset-and-sprom.md), `bcm_wifi/`, `scripts/proprietary.sh`.
