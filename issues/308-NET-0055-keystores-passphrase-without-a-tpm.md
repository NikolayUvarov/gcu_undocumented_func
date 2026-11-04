# 308-NET-0055 — `keystore`'s passphrase where no TPM seals the device key

**Type:** `keystore` · **Owner:** STO, by agreement with the network track's owner (the kernel session, 2026-11-04) · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [308](308-encryption-at-rest.md), for [308-STO-0028](308-STO-0028-a-recovery-key-and-a-passphrase.md) · **Constitution:** MC-6.8, MC-9.6, MC-11.9

**Numbered by the network track's owner** at STO's offer (`ALL/2026-11-04T0435Z-STO-…`): STO makes it with the track's code and counter (AGENTS.md, section 2), in `keystore`'s code, on its own branch.

## Problem

**Where a TPM seals the device key,** `keystore` unseals it at boot.

**Where none does,** the key is held unsealed today. A stolen disk then opens the store, and with it what `keystore` keeps for `tls`, `ssh` and `wlan`.

## Plan

1. **The device key sealed under the user's passphrase,** through Argon2id with recorded parameters, where no TPM seals it.
   - Argon2id is third-party code: an audited implementation with its licence, recorded in `THIRD_PARTY.md`.
2. **`keystore` holds the key only once the passphrase is typed.** Until then it answers that it waits, for `init` (308-KRN-0205).
3. **Changing the passphrase** seals the key again under the new one.

## Acceptance criteria

- **Host tests:** the seal and the unseal, a wrong passphrase refused, and the parameters read back.
- **In QEMU without a TPM:** the key is held only after the passphrase. With 308-KRN-0205, the keyed services start after it.

## Related

[308-KRN-0205](308-KRN-0205-the-boot-waits-for-keystores-passphrase.md), [308-KRN-0204](308-KRN-0204-a-store-client-that-may-recover-the-volume-key.md), [308-STO-0028](308-STO-0028-a-recovery-key-and-a-passphrase.md).
