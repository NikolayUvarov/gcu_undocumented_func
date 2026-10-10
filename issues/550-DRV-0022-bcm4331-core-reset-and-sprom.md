# 550-DRV-0022 — `bcm_wifi`, stage 1b: the 802.11 core held in reset, the SPROM read with its pins freed

**Type:** driver · **Owner:** `DRV` · **Priority:** P1 · **Status:** in progress (written; waits for a run on the MacBook Pro) · **Blocked by:** — · **Main task:** [550](550-network-on-real-hardware.md), through [550-DRV-0006](550-DRV-0006-broadcom-wifi.md) · **Constitution:** MC-1.5, MC-6.1, MC-12.1

## Problem

Stage 1 ([550-DRV-0020](../issues-done/550-DRV-0020-bcm4331-read-only-probe.done)) found two things on the MacBook Pro:

- **The 802.11 core runs.** The firmware left it out of reset with its clock on, and its PCI function has bus mastering on. Nothing of ours set it up, so whatever it receives may be written to memory the firmware no longer owns. The profile declares that no IOMMU is used (MC-1.5 not met), so nothing stops such a write.
- **The SPROM reads zeros.** Chip control `00010092` has bits 4 and 7 set, which give the SPROM's pins to the external amplifier lines. Without the SPROM there is no MAC address and no calibration data for later stages.

## Plan

`bcm_wifi/src/main.rs`, before anything else that touches the core:

- **The core.** If window 1 holds core 1 (the BCM4331's 802.11 core) and its wrapper says it runs, `bcm_wifi` sets the wrapper's reset bit and logs the state read back. The core stays in reset until a later stage sets it up.
- **The SPROM.** Chip control's bits 4, 7 and 12 (the amplifier lines) are cleared while the SPROM is read at both places, and the value read before is written back after. Both changes are logged.

These are the stage's only writes. The facts come from published register descriptions; no driver code is taken.

The time between the firmware's exit and `bcm_wifi`'s start is not covered: the kernel leaves bus mastering as the firmware set it until a driver is granted the device (a request in [requests-KRN.md](requests-KRN.md)).

## Acceptance criteria

On the MacBook Pro, one boot logs:
- the core held in reset (`NOW HELD IN RESET`), or that it was already;
- the SPROM either valid, with its revision and MAC address, or still not valid with the amplifier lines freed;
- chip control given back as it was.

The machine runs on as before. This is evidence for this machine only (MC-12.1).

## Related

[550-DRV-0006](550-DRV-0006-broadcom-wifi.md), [550-DRV-0020](../issues-done/550-DRV-0020-bcm4331-read-only-probe.done), `bcm_wifi/`.
