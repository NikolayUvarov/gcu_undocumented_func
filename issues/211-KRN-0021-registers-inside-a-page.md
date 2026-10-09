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

## Related

[211-DRV-0004](211-DRV-0004-ehci.md), [211-KRN-0019](211-KRN-0019-boot-logs-on-the-log-partition.md) (the log that showed it), [211-PRT-0004](211-PRT-0004-first-run-on-an-intel-pc.md).
