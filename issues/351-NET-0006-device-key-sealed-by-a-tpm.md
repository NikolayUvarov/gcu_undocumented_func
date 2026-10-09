# 351-NET-0006 — The device key sealed by a TPM

**Type:** network (key service) · **Owner:** `NET` track · **Priority:** P2 · **Status:** open · **Blocked by:** [351-DRV-0015](351-DRV-0015-tpm-driver.md) (a TPM 2.0 driver) · **Main task:** [351](351-self-update.md) · **Roadmap:** track D · **Constitution:** MC-11.9, Appendix B.6

Recorded by the storage session working the network track, at the maintainer's request (2026-10-09): the key on the disk ([351-NET-0005](../issues-done/351-NET-0005-persistent-device-key.done)) is the interim, a TPM next.

## Problem

The device key is stored unencrypted in `system/keystore`. Whoever has the disk itself, or boots another system on the machine, reads it.

## Plan

- **Sealing.** `keystore` seals the seed with the TPM (TPM 2.0: a sealed data object under a primary storage key of the owner hierarchy) and stores only the sealed blob in `system/keystore`. At boot it loads and unseals it. A copy of the disk alone no longer gives the key.
- **A policy on the boot state** (PCRs measured by the bootloader) is a later step: without it, any system booted on the same machine can unseal.
- **The transition:** a key stored unencrypted by 351-NET-0005 is sealed and the plain file removed. A machine without a TPM keeps the interim, and says so in its log.
- **Tests:** QEMU with `swtpm` (`tpm-crb` on x86, `tpm-tis-device` on aarch64):
  - the same key across boots;
  - the disk alone (the blob without the TPM state) does not unseal;
  - without a TPM, the interim.

## Acceptance criteria

With a TPM, the stored blob gives the key only on the machine whose TPM sealed it, and the key is the same across boots.

## Related

[351-NET-0005](../issues-done/351-NET-0005-persistent-device-key.done), [351-DRV-0015](351-DRV-0015-tpm-driver.md), [351-UPD-0011](351-UPD-0011-version-floor-in-the-tpm.md).
