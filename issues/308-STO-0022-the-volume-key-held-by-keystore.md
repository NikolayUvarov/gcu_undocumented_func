# 308-STO-0022 — The volume key held by `keystore`: sealed by the TPM or unlocked by a passphrase, with a recovery key

**Type:** storage (sealing at rest) · **Owner:** `STO` track · **Priority:** P1 · **Status:** open · **Blocked by:** [308-STO-0021](308-STO-0021-sealed-records.md); `keystore` and `init` are the kernel session's (`NET`, `KRN`): requests · **Main task:** [308](308-encryption-at-rest.md) · **Constitution:** MC-6.8, MC-9.6, MC-11.9

Split from [308](308-encryption-at-rest.md) (2026-10-28).

## Problem

After [308-STO-0021](308-STO-0021-sealed-records.md) the records are sealed, but the volume key lies in the superblock in the clear. Confidentiality at rest needs the key kept where a medium read elsewhere does not give it away (MC-11.9: access to key material apart from the right to use it).

## Plan

- **The superblock keeps the volume key wrapped,** never in the clear. `keystore` holds the wrapping key and unwraps the volume key for `blockstore`'s badge alone, at mount. `blockstore` keeps the volume key in memory while mounted and never writes it out.
- **Where the wrapping key comes from:**
  - sealed by the TPM where there is one (351-NET-0006);
  - unlocked by a passphrase at boot where there is none. Who asks for it is to be agreed with `KRN` and `APP`;
  - a recovery key the owner keeps, which unwraps the volume key on another machine.
- **Rotation:** a new wrapping key re-wraps the volume key. **Destruction:** the wrapping key goes, and the store is unreadable.
- **The recovery set of 305 depends on none of it** (MC-6.8).
- **The other tracks' parts**, as requests: `keystore.wit` (wrap and unwrap the store's key, for `blockstore`'s badge) and `init`'s grant of a `keystore` client to `blockstore`.

## Acceptance criteria

- QEMU, x86 and aarch64:
  - the sealed store mounts across a reboot with its key unwrapped by `keystore`;
  - a medium moved to another machine without the recovery key shows nothing;
  - the recovery key opens it.
- swtpm in QEMU: the wrapping key sealed by the TPM.
- docs/storage and the profile.

## Related

[308](308-encryption-at-rest.md), 351-NET-0006, 000-NET-0045, [305](../issues-done/305-recovery-without-the-store.done).
