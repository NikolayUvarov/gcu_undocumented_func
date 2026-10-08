# 210-APL-0008 — USB on the Type-C ports: DWC3 in host mode, the ATC PHY, PMGR power

**Type:** porting (drivers) · **Owner:** `APL` track (open) · **Priority:** P3 · **Status:** open · **Blocked by:** [210-APL-0003](210-APL-0003-aic-and-the-timer-fiq.md) (interrupts), [210-APL-0007](210-APL-0007-dart-dma-boundary.md) (DMA through the DART); a Mac with M1 ([issues-human](../issues-human/README.md#4-a-mac-with-apple-silicon-to-test-on)) · **Roadmap:** track H · **Constitution:** MC-1.3, MC-1.5, MC-12.1

Part of main task [210](210-apple-silicon-native.md), plan step 3.

## Problem

The first target of 210 needs an external USB keyboard and a USB stick. The Type-C ports of an Apple Silicon Mac are Synopsys DWC3 controllers (per Asahi's documentation). In host mode a DWC3 is an xHCI controller, which `usb_host` drives, but it is a device of the device tree (registers and an AIC line), not a PCI function as `usb_host` finds controllers now. Before it works, the port needs:

- its power domains, through the PMGR;
- its PHY, Apple's Type-C PHY (ATC);
- possibly the port's Type-C controller (a TI chip on I2C) for the data role and VBUS.

On a Mac mini the USB-A ports are expected to be on an xHCI on PCIe instead. That needs Apple's PCIe root ports and their DART; an alternative first target, to weigh when this task starts.

## Plan

- The xHCI of a DWC3 handed to `usb_host` from the device tree: its registers, line and DMA region given out by `init` from the board. `usb_host` is the drivers track's, which is open, so that change is made as a `DRV` task (AGENTS.md, section 5).
- The PMGR power domains and the ATC PHY in USB 2 mode first, which a keyboard and a stick need; USB 3 later.
- First see what m1n1 and U-Boot leave powered and set up (U-Boot is expected to use these ports itself); set up the rest here.
- From documentation only: Linux's drivers are GPL and cannot be copied into MIND Core (issue 210).

## Acceptance criteria

On an M1, a USB keyboard and a USB stick on a Type-C port work through `usb_host`, `usb_hid` and `usb_storage`, with their DMA through the DART; the system loads its programs from the stick. With 210-APL-0001 to 0007, this meets 210's acceptance on the first machine.

## Related

[210](210-apple-silicon-native.md), [164](../issues-done/164-usb-hid-keyboard-and-mouse.done) (`usb_host`, `usb_hid`, `usb_storage`), [210-APL-0007](210-APL-0007-dart-dma-boundary.md).
