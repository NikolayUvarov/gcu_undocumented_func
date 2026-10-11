# 173-KRN-0036 — A safe start: the service configuration ignored when a key is held at the bootloader

**Type:** kernel (ABI) and bootloader · **Owner:** `KRN`, with `PRT` (the bootloader's lines) · **Priority:** P2 · **Status:** open · **Blocked by:** [173-KRN-0035](173-KRN-0035-init-reads-the-service-configuration.md) · **Main task:** [173](173-boot-services-configuration.md) · **Constitution:** MC-6.1, MC-12.4

## Problem

A configuration that disables what a machine needs, such as its network on a machine reached over it, has to be undone without that service. The way back must not depend on the configuration itself.

## Plan

- The bootloader shows `MIND CORE BOOT: HOLD ESC FOR A SAFE START` during its pause and reads the keyboard through the firmware's text input.
- A key held there sets `BOOT_SAFE` in a flags field of `BootInfo`. That is an ABI change: a new version if ABI 4 is on `main` by then.
- With the flag set, init does not read `data/services.txt`, starts every service, and logs `[INIT] SAFE START: THE SERVICE CONFIGURATION IS IGNORED`. `boot-plan` says so.
- **Test:** QEMU sends the key through the monitor at the bootloader's pause. The configuration of 173-KRN-0035's test is then ignored.

## Acceptance criteria

The test passes on x86 and aarch64. Without the key the configuration applies as before.

## Related

[173](173-boot-services-configuration.md), [173-KRN-0035](173-KRN-0035-init-reads-the-service-configuration.md).
