# 104 — Modern VirtIO interface and MSI-X interrupts

**Type:** kernel + driver · **Owner:** kernel track · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Roadmap:** track A (stage III) · **Constitution:** MC-1.3, MC-6.3, Appendix B.6

## Problem

`virtio_net` (issue 100) uses the legacy VirtIO interface (I/O BAR0) and the legacy shared interrupt line. The legacy interface is deprecated, is missing on some hypervisors and on modern-only devices (device ID 0x1041), and shared INTx lines make a driver see other devices' interrupts. VirtIO block and input (track A) need the same groundwork.

## Plan

- **Kernel:** let a driver read the capability list of its own PCI function (read-only, no other configuration space), and add MSI-X: the kernel allocates vectors, programs the device's MSI-X table entries through a capability over the table BAR region and delivers each vector to an endpoint like `IRQ_BIND`; vectors are released when the capability is revoked. `STAT` reports them.
- **Driver library:** `mind::virtio` with the modern transport (common configuration, notification, ISR and device configuration structures in memory BARs, `FEATURES_OK`, `VIRTIO_F_VERSION_1`), shared by net, block and input drivers.
- **`virtio_net`:** prefers the modern interface, falls back to legacy; one vector per queue plus configuration changes.
- **init:** grants the BARs named by the VirtIO capabilities and the MSI-X capability instead of the INTx line.

## Acceptance criteria

- The `net` suite passes with `virtio-net-pci,disable-legacy=on` (modern only) and with a transitional device; interrupts arrive through MSI-X (`STAT` shows the vectors) and no INTx line is bound.
- A driver cannot program MSI-X entries of another device or read another function's configuration space (an `isolation` case).
- Restart: vectors of a killed driver are released and allocated again for the new instance.

## Related

[100](../issues-done/100-virtio-net-driver.done), [105](105-multiple-network-cards.md), [ROADMAP](../ROADMAP.md) track A.
