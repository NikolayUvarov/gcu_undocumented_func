# 210-APL-0007 — DART: each device's DMA only through its own IOMMU; 16 KiB pages

**Type:** porting (kernel) · **Owner:** `APL` track (open) · **Priority:** P3 · **Status:** open · **Blocked by:** [210-APL-0002](210-APL-0002-board-from-the-device-tree.md); `KRN` for DMA regions, which are the kernel's generic part (requested when this task starts); a Mac with M1 ([issues-human](../issues-human/README.md#4-a-mac-with-apple-silicon-to-test-on)) · **Roadmap:** track H · **Constitution:** MC-1.5, MC-1.8, MC-6.3, MC-12.1, MC-12.3

Part of main task [210](210-apple-silicon-native.md), plan steps 2 and 3.

## Problem

On an Apple Silicon Mac every DMA-capable device reaches memory only through its own DART, an IOMMU: a device cannot reach a page its DART does not map (per Asahi's documentation). The M1's DARTs use 16 KiB pages (same source); the kernel's pages are 4 KiB, a granule the M1's cores are expected to support.

Today no platform of MIND Core has a DMA boundary: MC-1.5 is "not met — declared" in both profiles, and every DMA driver is in the TCB of every memory guarantee. The DARTs are the first hardware that can change that.

## Plan

- The kernel programs the DARTs. A driver's DMA region (its DMA capability) is mapped in its device's DART, and nothing else is. A revoked region is unmapped and the DART's TLB invalidated before the memory is reused (MC-6.3, MC-1.8).
- 16 KiB: DMA regions on this platform are aligned and sized in 16 KiB units while CPU pages stay 4 KiB, or the kernel moves to a 16 KiB granule. Decided here with `KRN`, before the first driver uses DMA.
- Which DART serves which device, from the device tree (`iommus`); what m1n1 and U-Boot leave set (bypass or their own mappings).
- The profile's MC-1.5 row for the Mac: met only for the devices behind a DART the kernel programs, stated device by device (MC-12.3).
- The kernel's code is `PRT`'s directory: done with `PRT`.

## Acceptance criteria

On an M1, a DMA driver (the USB controller of [210-APL-0008](210-APL-0008-usb-on-type-c-ports.md)) works through its DART with only its own region mapped. A DMA address outside the region, set by a test, is refused by the DART and reported. Revoking the region stops the device's access before the memory is reused. The decision on 16 KiB pages is recorded here and in the profile.

## Related

[210](210-apple-silicon-native.md), [docs/profile/aarch64/](../docs/profile/aarch64/README.md) (MC-1.5 "not met — declared"), [150](../issues-done/150-user-memory-beyond-the-arena.done) (frame pool).
