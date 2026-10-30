# 308-STO-0028 — A recovery key and a passphrase for the store's volume key

**Type:** storage (sealing at rest) · **Owner:** `STO` track · **Priority:** P2 · **Status:** open · **Blocked by:** what `APP` and `KRN` agree to for the owner's input (below) · **Main task:** [308](308-encryption-at-rest.md) · **Constitution:** MC-6.8, MC-9.6, MC-11.9

Split from [308-STO-0022](../issues-done/308-STO-0022-the-volume-key-held-by-keystore.done) (2026-10-30): the part that needs the owner, once the volume key is off the medium.

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

## Progress (2026-10-30)

- **Built and host-tested: the store's own part of the recovery key.** Nothing is on the wire, so no program can make or use one yet; the wire (`blockstore.wit` 2.4) waits for the agreement with `APP` and `KRN` above. Described in [docs/storage](../docs/storage/README.md), "The recovery key".
  - `blockstore/src/store.rs`, a new key in two steps:
    1. `Store::recovery_new` makes 16 random bytes and a record that keeps the volume key sealed under them, flushes it and returns the key to be shown; nothing is erased;
    2. `recovery_keep` takes the key the owner types back and erases every record it does not open; a key that opens none is `key`, and nothing is erased.

    `recovery_forget` erases them all, and `recovery` counts them. `recover`, before a mount, opens a record with the owner's key and gives the volume key to that machine's keeper. A mount, a collection and a scrub leave the records alone.
  - `libmind/src/blockstore.rs`: `recovery_text` and `recovery_parse`, the form the owner writes down and types back: Crockford's base32 in 8 groups of 4, the last 4 bytes a check from SHA-256, so a typo is refused before any key is tried.
  - `tests/blockstore_host.rs`: recovery on a keeper without the key, a wrong key and a forged record, the text form, both steps of a rotation, a rotation cut after each write and flush, the record kept by a collection and unreported by a scrub, the refusals, and more records than a mount remembers.
- **Decision: a record in the log, not a copy in the superblock.** The plan above put the sealed copy in the superblock. But the store never writes over a stored sector, and a torn write of sector 0 would lose the whole medium. So the copy is a record of its own (frame domain 0xFFFE), written into blank sectors like any other, sealed under keys from HKDF-SHA-256 of the recovery key with the kept id as salt. The layout stays 4: a medium without such a record needs nothing. The first host-test criterion is met on this record in place of the superblock copy.
- **Decision: a rotation in two steps.** A new key's record is flushed, and the older ones are erased only once the owner types the new key back. So a cut, or a key shown but never written down, leaves the owner a key that opens the medium: the old one, or the new one once it was shown.
- **Changed from the proposal to `APP` and `KRN`.** The message `ALL/2026-10-30T1846Z-STO-a-recovery-key-and-a-passphrase-who-shows-and-who-asks.md` on `internal-dev-messages` proposed a copy in the superblock and a one-step rotation, `recovery-new` replacing that copy in one write. Both are changed here, as above. The wire it proposed will need a call for the second step (`recovery-keep`).
- **Still open:** the wire and who may call it; showing a key and taking it back (`APP`); the passphrase (`KRN`); the QEMU criteria; the profile.

## Related

[308](308-encryption-at-rest.md), [308-STO-0022](../issues-done/308-STO-0022-the-volume-key-held-by-keystore.done), 351-NET-0006, [305](../issues-done/305-recovery-without-the-store.done).
