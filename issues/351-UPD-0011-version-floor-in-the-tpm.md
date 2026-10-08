# 351-UPD-0011 — A version floor that someone with the disk cannot lower: a TPM 2.0 counter

**Type:** update (boot) · **Owner:** `UPD` track, with `PRT` for `bootloader/` · **Priority:** P2 · **Status:** open · **Blocked by:** [351-UPD-0009](351-UPD-0009-rollback-policy-and-key-roles.md) (the minimum version), [351-UPD-0012](351-UPD-0012-secure-boot-with-our-own-keys.md) (without it the counter can be bypassed) · **Main task:** [351](351-self-update.md) · **Constitution:** MC-9.4, MC-9.1

Asked by the maintainer (2026-10-08): what the limit of rollback protection is and what to do about it.

## Problem

Every release stays validly signed forever. So a signature alone does not stop anyone from installing an old release with a known hole. 351-UPD-0009 keeps the highest confirmed version and the channel's minimum in the boot records. That stops a network attacker and a stale mirror, but the records lie on the same disk as the slots. Someone with the disk — another OS booted from a stick, or the SSD in another machine — can write an old release and a lower minimum together. The floor has to live where it can only go up.

## Plan

- **The counter.** A TPM 2.0 NV index with the counter attribute (`TPMA_NV_COUNTER`): it can be increased and never decreased. Most Intel PCs since Haswell have a firmware TPM (Intel PTT), switched on in the firmware settings.
- **Only the bootloader touches it, through UEFI's `EFI_TCG2_PROTOCOL`** (`SubmitCommand`: `TPM2_NV_Read`, `TPM2_NV_Increment`) before `ExitBootServices`. The kernel needs no TPM driver.
- **The bootloader refuses a slot whose version is below the counter.**
- **The counter goes up at the next boot**, when the confirmed boot record asks for it, and only when both slots hold versions at or above the new floor. Otherwise raising it would leave no slot to fall back to (MC-9.3). The channel raises the minimum only for a security fix.
- **Defining the index** is a set-up step:
  - the image's first boot, or `update` with the maintainer's confirmation;
  - the index's authorization chosen so that the OS cannot define a second index in its place.
- **Without a TPM,** the profile states the limit: the floor protects against the network only.
- **Tests:**
  - QEMU with `swtpm` (`tpm-crb`) and OVMF with TCG2;
  - a slot below the counter is refused;
  - the counter rises only after confirmation;
  - a disk copied back with old records and an old slot does not boot the old slot.

## Acceptance criteria

In QEMU with a software TPM, an old release written to the disk together with old boot records is refused at boot once the counter has passed it. A normal update still falls back to the last-known-good slot. The profile's MC-9.4 row says on which machines the floor is in hardware.

## Related

[351](351-self-update.md), [351-UPD-0009](351-UPD-0009-rollback-policy-and-key-roles.md), [351-UPD-0012](351-UPD-0012-secure-boot-with-our-own-keys.md).
