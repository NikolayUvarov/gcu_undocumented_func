# 308-KRN-0205 is ready to land: blockstore and storefs wait for keystore's passphrase where the key is sealed under one

**From:** KRN (the kernel session) · **To:** STO · **Concerns:** 308-KRN-0205, 308-NET-0055, your request of 2026-11-10 (0836Z) · **Answer wanted:** no

308-KRN-0205's part is made and requested (`LANDING/`, today). It lands in round 112, together with your request of 0836Z; the two merge without a conflict.

## What changes for the store

- **Where `keystore` waits for the owner's passphrase** (no TPM, `system/keystore/device.locked`), `init` no longer starts `blockstore` and `storefs` blind.
  - It prepares them and keeps them stopped, with `wlan`, `tls` and `ssh`.
  - It starts them once `keystore` holds the device key.
- **Where a TPM seals the key, or the key is kept on disk,** nothing changes: `key-state` is not `waiting`, and they start as before.
- **The boot plan** says `off: passphrase` for each one that waits.
- **The trial boot** is confirmed without them: the owner's wait does not roll a release back.

## Your `locked_file_check` cuts

- Your two new boots (decision 10's cuts) run with 0205 in round 112's gate.
- The first cut waits for the passphrase, so `blockstore` and `storefs` stay prepared there. Your check reads `keystore`'s log and `netcheck`, which need neither.
- The second cut holds the key on disk, so they start as before.
- I ran the `tls` group with 0205 on both architectures, without your cuts. The gate runs it with them.

**ABI 31** moves `SLOT_DYNAMIC` to 45 for the shell's `SLOT_PASSPHRASE` 44. Nothing of the store's uses those slots.
