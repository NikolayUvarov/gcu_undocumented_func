# 550-KRN-0091 — `bcm_wifi` gets a DMA region and its interrupt: the card's MSI, granted by a new `PLATFORM_DEVICE_MSI`

**Type:** kernel (`kernel/src/pci.rs`, `scheduler.rs`), `common/abi.rs`, `init` · **Owner:** kernel session · **Priority:** P1 · **Status:** in progress (made, tested in QEMU; the MacBook Pro's run, by the drivers track's stage 4, left) · **Blocked by:** — · **Main task:** [550](550-network-on-real-hardware.md) · **Constitution:** MC-3.3, MC-8.3

## Problem

The drivers track's request (`requests-KRN.md`, 2026-10-12, for 550-DRV-0025, stage 4 after [550-DRV-0024](550-DRV-0024-bcm4331-phy-and-radio.md) of the MacBook Pro's Wi-Fi): to receive frames, the 802.11 core's DMA engines write them into rings in memory and raise an interrupt. `init` gave `bcm_wifi` only BAR0 and a read-only VFS client. The request:

- `SLOT_MEM`: one physically contiguous DMA region of 512 KiB below 4 GiB;
- `SLOT_IRQ`: the function's MSI (capability at 0x48, one vector, 64-bit), or its INTx line where MSI is not available.

The kernel could aim only MSI-X entries (`PLATFORM_DEVICE_MSIX`), and the BCM4331 has MSI, not MSI-X. Its INTx line would need the DSDT's `_PRT`, which is AML.

## Plan

- **`PLATFORM_DEVICE_MSI`** (13; a new kind, nothing else changes) gives an interrupt line 16 to 31. The kernel:
  - programs the device's MSI capability for one message (Multiple Message Enable 0), at the boot CPU's local APIC (on aarch64 through the ITS, as for MSI-X);
  - leaves its vector unmasked where the capability has masking;
  - turns it on.

  The vector stays with the device, as an MSI-X entry's does.
- **`init`'s `bcm_wifi` arm:**
  - a DMA region of 512 KiB in `SLOT_MEM`. The regions come from the kernel's arena, below 4 GiB, which the bootloader allocates there;
  - the MSI in `SLOT_IRQ`, else the INTx line, else none, with a line in the log.
  - Neither starts the core: the driver sets up its rings first.
- **The test kernel (`bar-move-test`)** grants QEMU's `edu` device its MSI (it has no MSI-X) and reads the capability back.

## Acceptance criteria

- **QEMU x86 (the `boot` suite with the test kernel).** `edu`'s MSI reads back as on, at 0xFEE…, with the data of the line granted (line 16, data 0x40).
- **The MacBook Pro.** `bcm_wifi` logs the region's physical address and size and waits on its interrupt; the drivers track checks it at stage 4.

## Progress

**2026-10-12, QEMU x86.** The `boot` suite passes with the test kernel: `edu`'s MSI is on, line 16, data 0x40.

## Related

[550-DRV-0024](550-DRV-0024-bcm4331-phy-and-radio.md), [550-DRV-0023](../issues-done/550-DRV-0023-bcm4331-microcode-runs.done), [docs/api/README.md](../docs/api/README.md).
