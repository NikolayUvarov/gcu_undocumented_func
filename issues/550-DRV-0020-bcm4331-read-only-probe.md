# 550-DRV-0020 — `bcm_wifi`, stage 1: a read-only probe of the MacBook Pro's BCM4331

**Type:** driver · **Owner:** `DRV` · **Priority:** P1 · **Status:** in progress (the driver is written and builds; it waits to be started) · **Blocked by:** `init` starting it, a request in [requests-KRN.md](requests-KRN.md) ("`bcm_wifi` as a boot service") · **Main task:** [550](550-network-on-real-hardware.md), through [550-DRV-0006](550-DRV-0006-broadcom-wifi.md) · **Constitution:** MC-12.1, MC-12.3

## Problem

Stage 1 of the Wi-Fi driver for the MacBook Pro's BCM4331 ([550-DRV-0006](550-DRV-0006-broadcom-wifi.md)). Nothing is known yet from the chip itself, only from its PCI header:

- which core BAR0's windows show;
- whether the 802.11 core is held in reset;
- whether a SPROM is fitted and readable, and the MAC address it holds.

Every later stage writes to the chip. This one must not: a wrong write to a Broadcom backplane can hang the machine, and the first run has no other way to report.

## Plan

`bcm_wifi/src/main.rs`, a ring-3 driver given the chip's BAR0 (16 KiB) in `SLOT_DEV0`. It reads only:

- **PCI configuration** (`DEVICE_CONFIG`): the IDs and the two window registers (0x80, 0xAC).
- **ChipCommon** at BAR0+0x3000: chip ID, revision, package, core count and bus type; capabilities, chip control and status, SROM control, the EROM pointer. A chip that is not a BCM4331 on an AI backplane stops the probe.
- **The windows:** window 1 must hold a core's base and window 2 that core's wrapper; otherwise the probe stops. The windows are not moved, since moving them needs a configuration write (a kernel change, later if needed).
- **The core's wrapper** at BAR0+0x1000: I/O control (its clock), I/O status, reset control and status. The core itself is not read while it may be in reset.
- **The PCIe core** at BAR0+0x2000: its first registers.
- **The SPROM,** only when ChipCommon says one is fitted. Both places it may appear are read with 16-bit reads, checked by revision and CRC-8, and the MAC address (revision 8: words 0x46–0x48) is logged from the valid one. An invalid read logs its first words, its last word and the CRC computed.

Every result goes to the log, which the run leaves on the log volume.

## Acceptance criteria

On the MacBook Pro, one boot logs the chip as `4331` on an AI backplane, the core in window 1 with its state, and either a valid SPROM with its MAC address or why none was read. The machine runs on as before. This is evidence of reads on this machine only (MC-12.1), not of the chip working.

## Related

[550-DRV-0006](550-DRV-0006-broadcom-wifi.md), [550](550-network-on-real-hardware.md), [211-PRT-0004](211-PRT-0004-first-run-on-an-intel-pc.md) (the machine), `bcm_wifi/`.
