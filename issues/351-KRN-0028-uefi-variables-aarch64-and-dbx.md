# 351-KRN-0028 — UEFI variables on aarch64, and authenticated writes for dbx

**Type:** kernel · **Owner:** `KRN`, with `PRT` (aarch64) · **Priority:** P3 · **Status:** in progress (aarch64 done 2026-10-09; dbx open) · **Blocked by:** — · **Main task:** [351](351-self-update.md) · **Constitution:** MC-9.3, MC-9.6

Split from [351-KRN-0027](../issues-done/351-KRN-0027-uefi-variables.done), which gave x86 `FIRMWARE_VARIABLE` for `BootNext`, `BootOrder` and `Boot####`.

## Problem

- **aarch64.** The kernel ignores the runtime services table on aarch64. `FIRMWARE_VARIABLE` answers `ERR_NOT_FOUND` there.
- **dbx updates.** [351-UPD-0012](351-UPD-0012-secure-boot-with-our-own-keys.md) revokes a bootloader by an authenticated append to `dbx` (`EFI_VARIABLE_AUTHENTICATION_2`, signed with our KEK). The call passes the attributes through, but:
  - a request holds at most 8 KiB (`FIRMWARE_BUFFER`), and a signed `dbx` update can be larger;
  - no test has tried an authenticated write.

## Plan

- **aarch64.** Map the runtime regions the memory map names in the kernel's tables. Call the services with the AAPCS64 convention, with the firmware's expected state (MMU attributes, no traps for FP).
  - AAVMF in QEMU first. A board whose firmware has no variable services (U-Boot's EFI without them) says so.
- **Authenticated writes.** Raise the request size, or let the buffer span pages, for a signed `dbx` append (`EFI_VARIABLE_APPEND_WRITE`).
  - Test in OVMF's Secure Boot build (`tests/secure_boot_smoke.py`): a signed append adds a hash; an unsigned one is refused (`ERR_RIGHTS`).

## Acceptance criteria

- On aarch64 QEMU with AAVMF, `efivar` lists the boot entries and `BootNext` boots another entry once.
- In OVMF with our keys, a `dbx` append signed with our KEK through `FIRMWARE_VARIABLE` stops the revoked bootloader at the next boot, and an unsigned one is refused.

## Progress

**2026-10-09: aarch64 done.**

- `kernel/src/firmware.rs` takes the runtime services table on aarch64 too. AAVMF's runtime code and data are already in the kernel's identity map as normal memory executable at EL1, and its flash as device memory, so nothing new is mapped.
- For the call the kernel allows FP/SIMD at EL1 (`CPACR_EL1.FPEN = 01`) and traps it again after. Programs are soft-float, so the firmware has no task state to clobber there.
- **Test:** the `efivar` suite runs on aarch64 (CI group "programs, shell and four CPUs"):
  - refused without consent;
  - with consent it lists AAVMF's entries (`Boot0006 EFI Internal Shell`);
  - `BootNext` to the shell boots it once, then MIND Core again with `BootNext` gone.
- The aarch64 profile names the variable services in its TCB.

Remaining: authenticated writes for `dbx` (larger requests, and a test in OVMF's Secure Boot build).

## Related

[351-KRN-0027](../issues-done/351-KRN-0027-uefi-variables.done), [351-UPD-0012](351-UPD-0012-secure-boot-with-our-own-keys.md), [351-KRN-0022](351-KRN-0022-updater-grants.md).
