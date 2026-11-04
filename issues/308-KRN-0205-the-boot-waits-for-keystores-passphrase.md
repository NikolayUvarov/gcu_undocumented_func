# 308-KRN-0205 — The boot waits for `keystore`'s passphrase before the services that need the device key

**Type:** `init` · **Owner:** kernel session · **Priority:** P2 · **Status:** open · **Blocked by:** [308-NET-0055](308-NET-0055-keystores-passphrase-without-a-tpm.md) · **Main task:** [308](308-encryption-at-rest.md), for [308-STO-0028](308-STO-0028-a-recovery-key-and-a-passphrase.md) · **Constitution:** MC-6.8, MC-9.6

## Problem

**Where a TPM seals the device key,** `keystore` unseals it at boot.

**Where none does,** the key will be sealed under the user's passphrase (308-NET-0055). Then `blockstore`, `tls`, `ssh` and `wlan` cannot start before the user types it.

## Plan

1. **`init`** starts the services that need the device key only once `keystore` says it holds it. It waits for that word from `keystore`, which waits for the passphrase.
2. **The console says why the boot waits.** The other services, and the shell, start as now.
3. **A safe start** asks for the passphrase as a normal one does.

## Acceptance criteria

- In QEMU without a TPM:
  - the store and the network's keyed services start only after the passphrase;
  - a wrong one leaves them unstarted, with the reason logged.
- With a TPM, the boot is as before.

## Related

[308-NET-0055](308-NET-0055-keystores-passphrase-without-a-tpm.md), [308-KRN-0204](308-KRN-0204-a-store-client-that-may-recover-the-volume-key.md), [308-STO-0028](308-STO-0028-a-recovery-key-and-a-passphrase.md).
