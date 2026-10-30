# 308-STO-0028 — A recovery key and a passphrase for the store's volume key

**Type:** storage (sealing at rest) · **Owner:** `STO` track · **Priority:** P2 · **Status:** open · **Blocked by:** what `APP` and `KRN` agree to for the owner's input (below) · **Main task:** [308](308-encryption-at-rest.md) · **Constitution:** MC-6.8, MC-9.6, MC-11.9

Split from [308-STO-0022](308-STO-0022-the-volume-key-held-by-keystore.md) (2026-10-30): the part that needs the owner, once the volume key is off the medium.

## Problem

Since 308-STO-0022 a medium's volume key is kept by `keystore`, under a key derived from the device key, which the TPM seals where there is one. A medium moved to another machine, or kept after its machine's device key was lost, shows nothing. That is the purpose of the change, but it leaves the owner no way back to their own data. Where there is no TPM, the device key lies on the boot disk, so whoever holds both disks holds the store.

## Plan

- **A recovery key** that the owner keeps away from the machine:
  - made at the store's format and shown to the owner once, in a form fit to write down;
  - the superblock keeps the volume key sealed under it, beside the kept key's id, never the recovery key itself;
  - on another machine, the recovery key unseals the volume key, which that machine's `keystore` then keeps; the medium mounts from then on as its own;
  - a new recovery key replaces the old copy in the superblock (rotation). A lost one is replaced from the mounted store.
- **A passphrase** where there is no TPM: the owner types it at boot, and `keystore` derives the key that seals its secrets from it and the device key. A wrong one opens nothing, and the store says that its key is not available rather than that the medium is damaged.
- **Who shows and who asks** is to be agreed:
  - with `APP`: the console's prompt, and how a recovery key is shown and typed back;
  - with `KRN`: `keystore`'s part of the passphrase (the kernel session's track) and any call the store needs.

  The requests go to `issues/requests-APP.md` and `issues/requests-KRN.md` once this plan is agreed.
- **The recovery set of 305 depends on none of it** (MC-6.8).

## Acceptance criteria

- Host tests: the superblock's sealed copy under a recovery key, its refusals, rotation.
- QEMU, x86 and aarch64:
  - a medium moved to another machine stays closed without the recovery key, and the recovery key opens it;
  - a wrong passphrase opens nothing, and the right one opens the store.
- docs/storage and the profile.

## Related

[308](308-encryption-at-rest.md), [308-STO-0022](308-STO-0022-the-volume-key-held-by-keystore.md), 351-NET-0006, [305](../issues-done/305-recovery-without-the-store.done).
