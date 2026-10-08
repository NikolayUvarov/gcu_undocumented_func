# 351-UPD-0012 — Secure Boot with our own keys, and old bootloaders revoked

**Type:** update (boot, root of trust) · **Owner:** `UPD` track, with `PRT` · **Priority:** P2 · **Status:** in progress · **Blocked by:** a run on the maintainer's PC ([issues-human](../issues-human/README.md), section 5) · **Main task:** [351](351-self-update.md) · **Constitution:** MC-9.1, MC-9.4, MC-9.6

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

## Progress (2026-10-08)

Worked by the storage session at the maintainer's request (2026-10-08).

- **Done in QEMU:**
  - `scripts/secure_boot.py`: test keys (PK, KEK and db, RSA-2048, labelled TEST), signing with `sbsign`, the Authenticode hash, and OVMF variable stores with the keys enrolled, Secure Boot on and revoked hashes in dbx (`python3-virt-firmware`).
  - `tests/secure_boot_smoke.py`, with OVMF's Secure Boot build: the bootloader signed with our db key boots to the shell; the firmware refuses (`Access Denied`) the unsigned bootloader, one signed with another key, and the signed one once its hash is in dbx. It runs in CI's "USB image" job and in `scripts/ci_local.sh`.
  - The builds sign the bootloader when `$MIND_SECURE_BOOT_KEYS` names the keys, before the boot manifest is signed.
  - **Revocation is chosen to be dbx**, not an SBAT-like generation with a TPM counter: the firmware enforces dbx without a TPM.
  - [docs/update/secure-boot.md](../docs/update/secure-boot.md) and [secure-boot_RU.md](../docs/update/secure-boot_RU.md) give the keys, the trust model and the steps for a real PC.
  - The profile's Article 9 row states it.
- **Open:**
  - the run on the maintainer's PC (issues-human, section 5), recorded with 211-PRT-0004;
  - authenticated dbx updates through the updater (351-UPD-0009, 0010);
  - Secure Boot on aarch64.

## Acceptance criteria

In QEMU with OVMF in Secure Boot mode and our keys:

- the signed bootloader boots the system;
- an unsigned or revoked one does not.

The steps for a real PC are written down and tried on the maintainer's machine (211-PRT-0004's PC), and the result is recorded.

## Related

[351](351-self-update.md), [351-UPD-0011](351-UPD-0011-version-floor-in-the-tpm.md), [350](350-signed-boot-images.md), [211](211-intel-pc-from-a-sata-ssd.md).
