# 351-UPD-0012 — Secure Boot with our own keys, and old bootloaders revoked

**Type:** update (boot, root of trust) · **Owner:** `UPD` track, with `PRT` · **Priority:** P2 · **Status:** open · **Blocked by:** 350-UPD-0003 (the bootloader verifies what it loads) · **Main task:** [351](351-self-update.md) · **Constitution:** MC-9.1, MC-9.4, MC-9.6

## Problem

Secure Boot must be off today, because the bootloader is unsigned. Anyone with physical access can then boot any EFI program. That includes:

- an old MIND Core bootloader that does not check the TPM floor of 351-UPD-0011;
- a modified one that skips every check.

Even a hardware counter is useless while the code that reads it can be replaced. The firmware has to run only our signed bootloader, and not its old, revoked versions.

## Plan

- **Signing.** `scripts/` signs `BOOTX64.EFI` (and `BOOTAA64.EFI`) with a Secure Boot key (`sbsign` or a Rust signer), kept apart from the release key (MC-9.6).
- **Enrolment on the machine.** The firmware's custom mode takes our own PK, KEK and db certificate. The machine then boots only what we sign, and no longer what Microsoft signs. Booting other operating systems needs their keys added deliberately. `docs/update.md` gives the steps for the maintainer's PC.
- **Revoking old bootloaders.** Their hashes go into `dbx` through an authenticated update signed by our KEK. As a lighter alternative, the bootloader carries a generation number compared with a second TPM counter (as shim's SBAT does). The choice is made in this task.
- **The trust model** goes into the profile:
  - the root of trust is the firmware's Secure Boot with our keys;
  - what that does not cover: the firmware itself, and an attacker who can change the firmware's settings, so a firmware password matters.
- **Tests:**
  - OVMF's Secure Boot build with our keys enrolled;
  - a signed loader boots;
  - an unsigned one and a revoked one are refused by the firmware.

## Acceptance criteria

In QEMU with OVMF in Secure Boot mode and our keys:

- the signed bootloader boots the system;
- an unsigned or revoked one does not.

The steps for a real PC are written down and tried on the maintainer's machine (211-PRT-0004's PC), and the result is recorded.

## Related

[351](351-self-update.md), [351-UPD-0011](351-UPD-0011-version-floor-in-the-tpm.md), [350](350-signed-boot-images.md), [211](211-intel-pc-from-a-sata-ssd.md).
