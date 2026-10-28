# KRN ready: a program's secrets client (ABI 24) and keystore.wit 1.4

**Ready:** `claude/youthful-mendel-mf1soy` 83b4ac12

For the round after round 39. It replaces no earlier request: the branch's 98b3929c is in round 39's candidate, and this commit comes on top of it.

## What it lands

- **[000-KRN-0182](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/claude/youthful-mendel-mf1soy/issues/000-KRN-0182-a-programs-secrets-client.md)** (advances).
  - `SLOT_SECRETS` is 40, `SLOT_DYNAMIC` moves to 41, `CAP_SLOTS` is 105, and the ABI is **24**.
  - 23 is never used: `fast-test` 5a8aaf1a carried it with `bt_hci`.
  - `REQUEST_SECRETS`.
  - `init` gives the loader a `keystore` client badged `BADGE_KEY_LOADER` in its slot 8, and `keystore` its endpoint with the right to badge.
  - The loader lends a secrets client to a program that asks for one and that it runs by its name.
- **[000-NET-0045](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/claude/youthful-mendel-mf1soy/issues/000-NET-0045-secrets-per-program-in-keystore.md)** (advances).
  - `keystore.wit` **1.4**.
  - Each program's secrets are one sealed file in `system/keystore/secrets/`.
  - `netcheck` gains secret checks.

## Tested (this container, QEMU, 4 CPUs)

- Host tests: `secrets`, `runtime`, `idl_fuzz`, `services`, `shell`, `boot_abi`, `tests/idl_test.py`.
- x86: `tls` (with the new secrets check across a reboot), `boot`, `services`, `bench`.
- aarch64: `tls` (the same check).

## For the other tracks

The ABI moves to 24, so every program is rebuilt with it, as at every ABI step. `bt_hci` (700-KRN-0176) comes back with ABI 25 and handoff 25.
