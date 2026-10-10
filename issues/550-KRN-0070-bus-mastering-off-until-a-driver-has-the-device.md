# 550-KRN-0070 — Bus mastering off at boot until a driver has the device

**Type:** kernel · **Owner:** kernel session · **Priority:** P1 · **Status:** in progress (made and tested in QEMU; the gate and the MacBook Pro's run left) · **Blocked by:** — · **Roadmap:** tracks A, D (main task [550](550-network-on-real-hardware.md)) · **Constitution:** MC-1.5, MC-6.3

## Problem

The drivers track's request, from the MacBook Pro's run of 550-DRV-0020:

- **What the kernel did.** It turned bus mastering on when it granted a device's resource to a driver (`pci::enable`) and off before a driver was restarted (`pci::quiesce`). Every other function's command register stayed as the firmware left it.
- **What the firmware leaves on the MacBook Pro.** The BCM4331 has bus mastering on and its 802.11 core running.
- **The window.** `bcm_wifi` holds that core in reset only about 5 s into the boot. Until then, and for any device that no driver is given, a device the firmware left running may write to memory.
- **What else could stop it.** The profile declares no IOMMU (MC-1.5), so nothing does.

## Plan

- **At enumeration.** `pci::enumerate` clears bus mastering (command bit 2) on every function the firmware left it on. It logs each one: `MIND CORE KERNEL: PCI: bb:dd.f CLASS cccccc: BUS MASTERING TURNED OFF UNTIL A DRIVER HAS IT`.
- **Left as they are:**
  - bridges (class 06), which carry their devices' DMA;
  - display controllers (class 03), which may scan out the boot screen from memory.
- **When a driver gets the function.** `pci::enable` turns bus mastering on again, as before.
- **The hardware report** marks each function it was turned off on: "(bus mastering, on at boot, turned off)".

## Acceptance criteria

1. **QEMU.**
   - The `x86: USB image` group reads this boot's hardware report from the log volume.
   - In that report no function that was not granted to a driver has bus mastering on, bridges and display controllers aside.
   - The drivers work as before (the gate's suites).
2. **The MacBook Pro.** The maintainer's run shows the same in `hw*.txt`. The BCM4331 is off until `bcm_wifi` starts.

## Progress

**2026-10-10: made, and run in QEMU x86 (`usb_image_smoke.py`).**
- Bus mastering was turned off on three functions: the IDE controller, the e1000, which has no driver, and the xHCI controller, which `usb_host` then gets.
- The report's check passes.

**Still exposed: display controllers.** Their bus mastering stays as the firmware left it, because turning it off could blank the boot screen. That is safe only once a display driver owns the device, or once the MacBook Pro shows the screen survives it.

## Related

[550-DRV-0022](../issues-done/550-DRV-0022-bcm4331-core-reset-and-sprom.done), 174-KRN-0043 (VT-d).
