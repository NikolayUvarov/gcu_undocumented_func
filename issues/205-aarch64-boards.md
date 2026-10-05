# 205 — aarch64 on boards with UEFI: Raspberry Pi 4/5 (EDK2), servers with ACPI

**Type:** porting · **Owner:** porting track · **Priority:** P2 · **Status:** open · **Blocked by:** — (201–204 done) · **Roadmap:** track H · **Constitution:** MC-12.1, Appendix A (profiles)

## Problem

MIND Core runs on aarch64 only in QEMU's `virt` machine (`aarch64/QEMU-virt-0`). Real machines differ in what the kernel takes as fixed today: the GIC's addresses (fixed for `virt` in `arch/aarch64/interrupts.rs`), the PL011 and PL031 (`arch/aarch64/platform.rs`), RAM above 4 GiB and an ECAM above it (the kernel's identity map covers 4 GiB), the PSCI conduit (SMC on hardware), GICv2 on the Raspberry Pi 4, and devices that are not VirtIO.

## Plan

- **Platform from ACPI:** the GIC distributor, redistributors and ITS from the MADT (GICD, GICR, ITS structures), the UART from the SPCR, the timer interrupt from the GTDT; `platform.rs` keeps only what a board's tables cannot say.
- **Memory:** map RAM and an ECAM above 4 GiB (the identity map grows, or the kernel maps device windows on demand).
- **GICv2** (Raspberry Pi 4: GIC-400) beside GICv3.
- **Drivers:** NVMe or USB storage on the board's PCIe/xHCI (`usb_storage` exists for xHCI), a USB keyboard (HID), the GOP framebuffer as on QEMU; a network card (the Pi's GENET or a USB adapter) later.
- **Profiles:** one per board family (`aarch64/RPi4-EDK2-0`, `aarch64/ACPI-server-0`), with their TCB (the board's firmware) and evidence on hardware.

## Acceptance criteria

A Raspberry Pi 4 with the EDK2 port (or an ACPI aarch64 server, or QEMU `sbsa-ref` as a stand-in) boots the same image to the shell, reads its boot disk and takes keyboard input; its profile lists what was tested on hardware.

## Related

[201](../issues-done/201-aarch64-boot.done), [202](../issues-done/202-aarch64-devices.done), [203](../issues-done/203-aarch64-smp-and-power.done), [204](../issues-done/204-aarch64-profile-and-ci.done), [docs/profile/aarch64/](../docs/profile/aarch64/README.md).
