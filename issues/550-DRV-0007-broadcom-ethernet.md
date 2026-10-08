# 550-DRV-0007 — Broadcom tg3-family Ethernet: the Thunderbolt Gigabit Ethernet adapter and PCs

**Type:** driver · **Owner:** `DRV` (open) · **Priority:** P3 · **Status:** open (proposed) · **Blocked by:** hardware with such a chip: the Mac with a Thunderbolt adapter, or a PC (QEMU has no model) · **Main task:** [550](550-network-on-real-hardware.md) · **Constitution:** MC-12.1, MC-1.5, Appendix B.6

Numbered by the kernel session at the maintainer's request (2026-10-08); the track is open.

## Problem

Broadcom's NetXtreme gigabit chips, which Linux drives with `tg3`, are common:

- Apple's Thunderbolt to Gigabit Ethernet adapter uses one (a BCM57762, as Linux identifies it);
- many PCs do, mostly business desktops and laptops, and servers.

The MacBook Pro's own Ethernet function, `03:00.0`, is of the same family (BCM57765), but this model has no socket for it. MIND Core has no driver for any of these chips, and QEMU emulates none of them, so nothing in this task can be tested in CI.

## Plan

- **A ring-3 PCIe driver** (`bcm_eth`, proposed name) for the family's later chips first (57762, 57765, the 5717 series):
  - sources: Broadcom's programmer's reference guides where they are public. Linux's `tg3` (GPL) and the BSDs' `bge` are read for behaviour, not copied;
  - chip reset and identification; the MAC address from the chip's NVRAM or registers;
  - the PHY and the link: auto-negotiation, link changes by interrupt;
  - the receive producer and return rings, the send ring and the status block; MSI or MSI-X as the kernel offers them;
  - serves `idl/net.wit` 1.2 as `virtio_net` does. Offloads stay off until measured, as issue 106 decided.
- **Firmware.** Linux loads a firmware patch for a few chips of the family. Whether the chips here need one is checked first, and any file goes through [THIRD_PARTY.md](../THIRD_PARTY.md).
- **Thunderbolt on the Mac.** The adapter is expected to appear as a PCIe device only when it is plugged in before power-on: Apple's firmware then sets up the tunnel, as users of Linux on these Macs report. That the kernel's PCI scan finds it behind the Thunderbolt bridges is checked on the first run. Hot plug needs a Thunderbolt driver and is not part of this task.
- **On the Mac without an adapter,** the driver can start on `03:00.0` to check identification and register access, though there is no link.
- **DMA.** No IOMMU is used, so the driver joins the TCB of memory isolation (MC-1.5), and [docs/profile/tcb.md](../docs/profile/tcb.md) says so when it lands.
- **`init`** starts one instance per matching device, as for `virtio_net` (issue 105); that part is `KRN`'s, with 550-DRV-0005's.

## Acceptance criteria

- On a machine with such a chip, with the chip's IDs recorded: a DHCP lease and the shell's `https` fetch; the link is seen to go down and come back when the cable is pulled out and put back.
- The profile records the result for that machine only. No CI suite is claimed, since QEMU has no model.

## Related

[550](550-network-on-real-hardware.md), [550-DRV-0005](550-DRV-0005-usb-ethernet.md), [105](../issues-done/105-multiple-network-cards.done), [106](../issues-done/106-network-offloads.done), `virtio_net`, `idl/net.wit`, [211-PRT-0004](211-PRT-0004-first-run-on-an-intel-pc.md).
