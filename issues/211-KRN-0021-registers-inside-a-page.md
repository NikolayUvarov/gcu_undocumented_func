# 211-KRN-0021 — Device registers that start inside a page (Apple's EHCI)

**Type:** kernel · **Owner:** `KRN` · **Priority:** P1 · **Status:** in progress · **Blocked by:** — · **Main task:** [211](211-intel-pc-from-a-sata-ssd.md) · **Constitution:** MC-3.9, MC-12.1

## Problem

On the MacBook Pro of [211-PRT-0004](211-PRT-0004-first-run-on-an-intel-pc.md), `usb_host` logged nothing about its EHCI controllers ([211-DRV-0004](211-DRV-0004-ehci.md)), although init finds two (`00:1a.0`, `00:1d.0`, a BAR of 1 KiB each). The internal keyboard and trackpad sit behind them and stayed silent.

Every place where that start can fail was silent:

- init skipped an EHCI whose BAR it could not mint;
- `usb_host` skipped one whose registers or DMA region it could not map.

The likely cause is in the kernel:

- a BAR becomes an MMIO capability at its own base;
- `MEM_MAP` maps only from a page boundary;
- firmware may pack small BARs into one page, for example a 1 KiB BAR at `…C00`, and Apple's is expected to;
- such a BAR could never be mapped.

In QEMU every BAR starts on a page.

## Plan

- **Mapping.** `MEM_MAP` of an MMIO capability that starts inside a page maps that page and the ones after it that the capability covers. It returns the address of the first register. `FREE` takes that address for the mapping too. Capabilities that start on a page are mapped as before.
- **Isolation.** The page may hold registers of other devices.
  - Once a BAR is granted, devices of another class with registers in its pages are granted to no other driver.
  - A BAR whose pages hold registers of a device already granted is refused.
  - Each case is a kernel line naming both devices. Devices of the same class, such as the two EHCI controllers, may share a page; both go to `usb_host`.
- **What failed, in the log.**
  - init logs an EHCI whose registers it could not grant, with the error.
  - `usb_host` logs each EHCI's vendor and device ID and register address, and the error if mapping fails.

## Acceptance criteria

- **QEMU.** Every suite passes: their BARs start on a page, and nothing changes for them.
- **The MacBook Pro.**
  - `usb_host` logs both EHCI controllers with their register addresses, and their ports.
  - The kernel line names any device that shares their page.

## Progress

**2026-10-08 (dce9d65):**
- `MEM_MAP` maps registers that start inside a page.
- The isolation rule for shared pages is in place.
- init and `usb_host` log EHCI start failures.

**2026-10-09: the MacBook Pro's logs (`LOG:boot0001.log`, `boot0002.log`).**
- Both boots logged `[INIT] EHCI 0: REGISTERS NOT GRANTED (Rights)`, and the same for EHCI 1.
- The kernel refused both BARs: their page holds registers of a device of another class that was granted first. `ahci` (00:1f.2, whose 2 KiB ABAR is the likely neighbour) started 30 ms before.
- The kernel's line naming the neighbour went only to COM1. The Mac has none, and by then the compositor had the screen.
- So the internal keyboard and trackpad stayed silent. The routing line shows all 4 USB 2 and 4 USB 3 ports moved to xHCI, so they sit on EHCI's other ports.

**The fix: a BAR moves to a page of its own.**
- When a memory BAR smaller than a page is granted while its page holds registers of another kind of device, the kernel first moves it to a free 4 KiB page, and logs `PCI: BAR n OF … MOVED FROM … TO …: ITS PAGE HELD REGISTERS OF …`.
- A free page is one:
  - inside the span the firmware gave bus 0's devices, so the chipset decodes it;
  - below the fixed ranges at 0xFEC0_0000;
  - clear of every BAR, every bridge's memory windows, the ECAM (now read from the MCFG on x86 too) and every range of the firmware's memory map.
- The move is made with decoding off and checked by reading the BAR back.
- The old rule (refuse or mark) still applies when no page is free, or when the device already holds a grant. On the Mac the first of AHCI and the EHCIs to be granted, AHCI, moves, and the EHCIs keep their shared page, which the same class may share.
- Bridges are now sized with their two BARs only. The rest of a type-1 header (bus numbers, windows) was read as four more BARs.
- **Test.** The kernel built with `bar-move-test` (boot suite, x86) packs the RTL8139's 256-byte BAR into the SD host controller's page, as the firmware packs EHCI next to AHCI. Granted, the BAR moves to a page of its own, and the card's MAC address reads there. OVMF gives every small BAR a page of its own, so QEMU cannot show the packing otherwise.

Left: the acceptance criteria on the MacBook Pro. `usb_host` should log both EHCIs, with their register addresses and ports.

## Related

[211-DRV-0004](211-DRV-0004-ehci.md), [211-KRN-0019](211-KRN-0019-boot-logs-on-the-log-partition.md) (the log that showed it), [211-PRT-0004](211-PRT-0004-first-run-on-an-intel-pc.md).
