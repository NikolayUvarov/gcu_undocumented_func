# 211-KRN-0187 — A second `rtl8168` for the DEXP's second wired port

**Type:** `init`, `common/abi.rs` (`SERVICE_INSTANCES`) · **Owner:** `KRN` · **Priority:** P1 (the maintainer's order for the DEXP, 2026-10-27) · **Status:** in progress (2026-10-30: made; the DEXP's run is DRV's) · **Blocked by:** — · **Main task:** [211](211-intel-pc-from-a-sata-ssd.md) · **Constitution:** MC-2.3, MC-3.1, MC-6.3

**Asked by:** the drivers track (`DRV`) for [211-DRV-0041](211-DRV-0041-the-dexps-rtl8168h.md), in `KRN/2026-10-27T1530Z-DRV-a-second-rtl8168-and-the-dexps-wi-fi.md` on `internal-dev-messages`, point 1. The kernel track left it unanswered until 2026-10-30.

## Problem

- **The DEXP has two RTL8168H ports,** PCI 01:00.0 and 04:00.0, both `10ec:8168`.
- **`init` starts one `rtl8168`,** for the first `10ec:8168` it finds (`find_device_id(..., 0)`), so the second port has no driver and `netstack` no second card.

## Plan

- **A further instance `rtl8168#1`** in `SERVICE_INSTANCES`, as `virtio_net#1` is for a second VirtIO card. It drives the second `10ec:8168` in PCI order, with its own registers, DMA region, MSI and endpoint. The driver itself needs no change: it serves the device it is given.
- **`init`:**
  - the `rtl8168` grant takes the instance's device;
  - restarts use the image's name, so the instance's registers are granted again without mastering;
  - `netstack` takes the wired cards in the order `virtio_net`, `virtio_net#1`, `rtl8168`, `rtl8168#1`, two at most (slots 2 and 3).
- **No ABI step:** neither the system calls nor `BootInfo` change. `init.wit`'s `list` names one more unit.

## Progress (2026-10-30)

- **Made:**
  - `SERVICE_INSTANCES` holds `rtl8168#1`;
  - `init`'s `rtl8168` grant takes the instance's device in PCI order;
  - a restart grants either instance's registers without mastering;
  - `netstack`'s wired cards are `virtio_net`, `virtio_net#1`, `rtl8168`, `rtl8168#1`, two at most.
- **Docs:** the README's and the profile's rows name the instance.

## Acceptance criteria

- **Host tests:** `tests/services_host.rs` with the new unit, `netstack` after it; `tests/svc_host.rs`.
- **QEMU, x86:** the boot and services suites pass; without an RTL8168 both instances say `NOT STARTED: NO DEVICE`.
- **On the DEXP** (DRV's run, 211-DRV-0041): `rtl8168#1` drives 04:00.0, and `netstack` reaches both ports. The machine and the build are named (MC-12.1).

## Related

[211-DRV-0041](211-DRV-0041-the-dexps-rtl8168h.md), [213-DRV-0031](213-DRV-0031-rtl8168-wired-network.md), [550-KRN-0172](550-KRN-0172-rtw88-wifi-boot-image-and-grant.md) (point 2 of the same request).
