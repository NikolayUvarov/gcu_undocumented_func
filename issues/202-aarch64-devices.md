# 202 — aarch64 devices: PCIe ECAM, VirtIO, PL011, PL031, display

**Type:** drivers · **Owner:** porting track (VirtIO block and input with track A) · **Priority:** P2 · **Status:** open · **Blocked by:** 201 · **Roadmap:** tracks H and A · **Constitution:** Appendix B.6, MC-6.3

## Plan

- **PCI:** configuration through ECAM, from ACPI MCFG or the device tree. Device, BAR, MSI-X capabilities as on x86; MSI-X goes through the GICv3 ITS.
- **VirtIO over PCI, written once and used on both architectures:**
  - `virtio_net` (exists, MMIO BARs);
  - `virtio_blk` (new, track A), replacing ATA/AHCI on `virt`;
  - `virtio_input`, the keyboard and the pointer, replacing PS/2;
  - `virtio_rng`, entropy without `RNDR`.
- **Console and clock:** the shell's console on the PL011 (in place of COM1); `rtc` on the PL031.
- **Display:** the GOP framebuffer from 201, so the compositor works unchanged.
- **Sound:** later (virtio-snd).

## Acceptance criteria

The `normal`, `shell`, `vfs`, `net` and `tls` suites pass on `qemu-system-aarch64 -machine virt` with virtio-blk, virtio-net, virtio-keyboard and ramfb.

## Related

[201](201-aarch64-boot.md), [204](204-aarch64-profile-and-ci.md), [ROADMAP](../ROADMAP.md) track A.
