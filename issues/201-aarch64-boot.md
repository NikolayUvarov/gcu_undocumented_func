# 201 — aarch64 on QEMU `virt`: boot to init

**Type:** kernel + bootloader · **Owner:** porting track · **Priority:** P2 · **Status:** open · **Blocked by:** 200 · **Roadmap:** track H · **Constitution:** MC-1.x (isolation on the new platform), MC-12.1

## Problem

MIND Core runs only on x86-64.

## Plan

- **Bootloader:** `aarch64-unknown-uefi` with the same `uefi` crate. It loads the kernel and the boot images, takes the GOP framebuffer (ramfb or virtio-gpu under EDK2) and the ACPI RSDP or device tree, and exits boot services.
- **Kernel** (`arch/aarch64`, target `aarch64-unknown-none-softfloat` like the soft-float x86 kernel):
  - EL1 exception vectors; the ARMv8 MMU (4 KiB granule, 4 levels, PAN/PXN for user pages, NX like on x86); context switch;
  - `svc` system calls with the same mailbox ABI;
  - the generic timer for ticks and the monotonic clock;
  - GICv3 for interrupts, routed as the existing IRQ capabilities;
  - the PL011 UART for the kernel's boot and panic lines.
- **libmind:** `svc` stub, `CNTVCT_EL0` as the cycle counter, `RNDR` when present (else no entropy: TLS fails closed as on x86 without RDRAND).
- **Programs:** built for the aarch64 target with their unchanged sources. ELF loader and relocations for `R_AARCH64_*`.

## Acceptance criteria

- `qemu-system-aarch64 -machine virt -cpu max` with AAVMF/EDK2 boots to `[INIT] READY` on the PL011 console.
- init starts the services that need no devices (`logd`, `loader`, `sysmon`, `keystore`).
- An application fault is contained (`isolation` cases that need no devices).

## Related

[200](200-architecture-layer.md), [202](202-aarch64-devices.md), [203](203-aarch64-smp-and-power.md).
