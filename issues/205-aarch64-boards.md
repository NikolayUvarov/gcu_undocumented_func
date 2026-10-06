# 205 — aarch64 on boards with UEFI: Raspberry Pi 4/5 (EDK2), servers with ACPI

**Type:** porting · **Owner:** porting track · **Priority:** P2 · **Status:** open (steps 1–4 done 2026-10-05) · **Blocked by:** — (201–204 done) · **Roadmap:** track H · **Constitution:** MC-12.1, Appendix A (profiles)

## Problem

MIND Core runs on aarch64 only in QEMU's `virt` machine (`aarch64/QEMU-virt-0`). Real machines differ in what the kernel takes as fixed today: the GIC's addresses (fixed for `virt` in `arch/aarch64/interrupts.rs`), the PL011 and PL031 (`arch/aarch64/platform.rs`), RAM above 4 GiB and an ECAM above it (the kernel's identity map covers 4 GiB), the PSCI conduit (SMC on hardware), GICv2 on the Raspberry Pi 4, and devices that are not VirtIO.

## Plan

- **Platform from ACPI:** the GIC distributor, redistributors and ITS from the MADT (GICD, GICR, ITS structures), the UART from the SPCR, the timer interrupt from the GTDT; `platform.rs` keeps only what a board's tables cannot say.
- **Memory:** map RAM and an ECAM above 4 GiB (the identity map grows, or the kernel maps device windows on demand).
- **GICv2** (Raspberry Pi 4: GIC-400) beside GICv3.
- **Drivers:** NVMe or USB storage on the board's PCIe/xHCI (`usb_storage` exists for xHCI), a USB keyboard (HID), the GOP framebuffer as on QEMU; a network card (the Pi's GENET or a USB adapter) later.
- **Profiles:** one per board family (`aarch64/RPi4-EDK2-0`, `aarch64/ACPI-server-0`), with their TCB (the board's firmware) and evidence on hardware.

## Progress (2026-10-05)

- **Done — platform from ACPI:** `arch/aarch64/acpi.rs` reads the MADT (GICC, GICD, GICv2m, GICR, ITS), SPCR, GTDT, MCFG and the FADT's PSCI conduit into `arch/aarch64/board.rs`; `virt`'s values are only the defaults. The kernel prints the layout it found (`MIND CORE KERNEL: BOARD …`).
- **Done — memory:** the identity map covers 1 TiB with memory types from the UEFI map; the ECAM comes from the MCFG. Tested with `-m 6G` and `highmem=on` (normal, net, tls).
- **Done — GICv2:** GICC CPU interface, SGIs through GICD_SGIR, MSIs through GICv2m. Tested with `gic-version=2` (normal, shell, smp, net).
- **Done — NVMe:** the `nvme` service (admin and one I/O queue, polled, PRP lists) is a boot disk on both architectures; CI's "NVMe boot disk" groups.
- **Done — USB keyboard** (the boards have no PS/2 or virtio-input): issue [164](../issues-done/164-usb-hid-keyboard-and-mouse.done), tested on `virt` with `qemu-xhci`. On the Raspberry Pi 4 its xHCI (VL805) sits behind a PCIe root that is not a standard ECAM: open.
- **Done — no console at a guessed address:** the UART and RTC start as none and get `virt`'s addresses only on QEMU (XSDT OEM ID `BOCHS`); an SPCR naming a UART the kernel does not drive (a 16550, the Pi's mini UART) leaves no console instead of writes that may land in RAM.
- **Open — hardware or `sbsa-ref`:** the SBSA firmware is not packaged and its download needs an account, so the acceptance run could not be made here.
- **Open — board profiles** (`aarch64/RPi4-EDK2-0`, `aarch64/ACPI-server-0`) wait for that run.

## Acceptance criteria

A Raspberry Pi 4 with the EDK2 port (or an ACPI aarch64 server, or QEMU `sbsa-ref` as a stand-in) boots the same image to the shell, reads its boot disk and takes keyboard input; its profile lists what was tested on hardware.

## Related

[201](../issues-done/201-aarch64-boot.done), [202](../issues-done/202-aarch64-devices.done), [203](../issues-done/203-aarch64-smp-and-power.done), [204](../issues-done/204-aarch64-profile-and-ci.done), [docs/profile/aarch64/](../docs/profile/aarch64/README.md).
