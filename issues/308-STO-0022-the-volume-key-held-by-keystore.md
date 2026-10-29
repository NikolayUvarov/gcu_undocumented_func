# 308-STO-0022 — The volume key held by `keystore`: sealed by the TPM or unlocked by a passphrase, with a recovery key

**Type:** storage (sealing at rest) · **Owner:** `STO` track · **Priority:** P1 · **Status:** open · **Blocked by:** [308-KRN-0183](308-KRN-0183-a-secrets-client-for-blockstore.md) (the block store's secrets client from `init` and `keystore`) · **Main task:** [308](308-encryption-at-rest.md) · **Constitution:** MC-6.8, MC-9.6, MC-11.9

Split from [308](308-encryption-at-rest.md) (2026-10-28).

## Problem

After [308-STO-0021](../issues-done/308-STO-0021-sealed-records.done) the records are sealed, but the volume key lies in the superblock in the clear. Confidentiality at rest needs the key kept where a medium read elsewhere does not give it away (MC-11.9: access to key material apart from the right to use it).

## Plan

- **The superblock keeps the volume key wrapped,** never in the clear. `keystore` holds the wrapping key and unwraps the volume key for `blockstore`'s badge alone, at mount. `blockstore` keeps the volume key in memory while mounted and never writes it out.
- **Where the wrapping key comes from:**
  - sealed by the TPM where there is one (351-NET-0006);
  - unlocked by a passphrase at boot where there is none. Who asks for it is to be agreed with `KRN` and `APP`;
  - a recovery key the owner keeps, which unwraps the volume key on another machine.
- **Rotation:** a new wrapping key re-wraps the volume key. **Destruction:** the wrapping key goes, and the store is unreadable.
- **The recovery set of 305 depends on none of it** (MC-6.8).
- **The other tracks' parts**, as requests: `keystore.wit` (wrap and unwrap the store's key, for `blockstore`'s badge) and `init`'s grant of a `keystore` client to `blockstore`.
- **Changed on 2026-10-29: the store's own secret, no wrap call.** `keystore.wit` 1.4 (000-NET-0045, the kernel session's, landing) keeps a program's secrets sealed under a key derived from the device key, and the TPM seals that key once 351-NET-0006 is active. So:
  - the volume key is `blockstore`'s secret `volume-<id>`; the superblock keeps the id and the check (key source 2), never the key;
  - `keystore` keeps its own key, and `blockstore` holds the volume key in memory while mounted, as it must to seal;
  - the only other part is a secrets client for the store, from `init`, under a name no loaded program can take ([requests-KRN.md](requests-KRN.md));
  - a RAM disk's key is kept in memory only: nothing outlives a reset there.
  - The recovery key and the passphrase stay as planned, and need `APP` for what the owner sees and types; they come after the key is off the medium.

## Progress (2026-10-29)

- **The store's side is built and host-tested,** against `keystore.wit` 1.4's `secret-put` and `secret-get` on a client in `blockstore`'s slot 3, as 308-KRN-0183 plans it.
  - **Key source 2 (kept):** the superblock holds an 8-byte id and the check, never the key; the key is the secret `volume-<id in hex>`.
  - **Which media:** a RAM disk keeps its key in its superblock, as nothing of it outlives a reset and a restarted store must find it. Any other medium's key is kept when it is random and `keystore` keeps it.
  - **Not kept:** a derived key, as it could be found again from the medium's size and the time; and a key `keystore` refuses, which stays in the clear and is logged. A medium formatted with its key in the clear keeps it there: no conversion.
  - **Refused at mount:** a kept key's medium without the client, or whose secret is missing (another machine's medium), with `key` on the wire and the secret named in the log.
- **Until `init` lends the client,** every medium keeps its key in the clear and says why, so the platform's behaviour does not change yet. The QEMU criteria below wait for 308-KRN-0183.
- **The recovery key and the passphrase** are still to come, with `APP`.
- **Recorded in [docs/storage](../docs/storage/README.md)** (0.16). The profile does not change until the platform runs it.

## Acceptance criteria

- QEMU, x86 and aarch64:
  - the sealed store mounts across a reboot with its key unwrapped by `keystore`;
  - a medium moved to another machine without the recovery key shows nothing;
  - the recovery key opens it.
- swtpm in QEMU: the wrapping key sealed by the TPM.
- docs/storage and the profile.

## Related

[308](308-encryption-at-rest.md), 351-NET-0006, 000-NET-0045, [305](../issues-done/305-recovery-without-the-store.done).
