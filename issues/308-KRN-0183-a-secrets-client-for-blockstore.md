# 308-KRN-0183 — A secrets client for `blockstore`, under a name no program can take

**Type:** `init` (grants), `keystore` (`idl/keystore.wit`) · **Owner:** `KRN` (with `NET`'s `keystore`, which the kernel session holds) · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Main task:** [308](308-encryption-at-rest.md) (STO's), for [308-STO-0022](308-STO-0022-the-volume-key-held-by-keystore.md) · **Constitution:** MC-3.1, MC-6.8, MC-10.3, MC-11.9, MC-12.4

**Asked by:** the storage track (`STO`) in `issues/requests-KRN.md`, 2026-10-29, "The block store's volume key kept by `keystore` (STO, for 308-STO-0022)".

## Problem

- **The store's volume key is in its superblock in the clear** ([308-STO-0021](../issues-done/308-STO-0021-sealed-records.done) sealed the records, not the key). Whoever reads the medium opens the records.
- **`keystore` already keeps a program's secrets** (`keystore.wit` 1.4, [000-NET-0045](../issues-done/000-NET-0045-secrets-per-program-in-keystore.done)), but only for a program the loader runs by its name ([000-KRN-0182](../issues-done/000-KRN-0182-a-programs-secrets-client.done)). A boot service gets no such client.
- **A program's name is not enough for a service.** A program called `blockstore` in `data/apps/` would get the client named `blockstore`, so the store's secrets need a namespace no loaded program reaches.

## Plan

- **`keystore.wit`, its next minor version (1.5):**
  - a service's secrets client, minted only for `init`'s own badge at `keystore` (`BADGE_KEY_INIT`), for a service `init` names;
  - kept apart from programs': `system/keystore/services/<name>.kept`, and the sealing key's label names a service, so neither a program's name nor a file copied between the two directories opens it.
  - The calls on the client are 1.4's (`secret-put`, `secret-get`, `secret-delete`, `secret-list`).
- **`init`:**
  - holds a `keystore` client badged `BADGE_KEY_INIT`, apart from the loader's `BADGE_KEY_LOADER`;
  - after `keystore` is ready, asks for `blockstore`'s client and puts it in `blockstore`'s slot 3, before `blockstore` mounts;
  - when `keystore` gives none, starts `blockstore` without it and logs why (MC-6.8): the store depends on `keystore`, never the other way round.
- **Docs:** `idl/keystore.wit`, `docs/profile/bootstrap.md` (`blockstore`'s slot 3, `init`'s client), the profile's MC-11.9 row.

## Acceptance criteria

- **Host tests:** a service's name and a program's name never meet (the paths, the sealing label).
- **QEMU, x86 and aarch64:** `blockstore` holds a secrets client from `init` at boot; a secret it keeps through it is read back after a reboot; a program named `blockstore` run from `data/apps/` gets a program's client that does not see it.
- **The other suites** pass. The storage track's suites check the rest once the store uses the client (308-STO-0022).

## Related

[308-STO-0022](308-STO-0022-the-volume-key-held-by-keystore.md), [000-KRN-0182](../issues-done/000-KRN-0182-a-programs-secrets-client.done), [000-NET-0045](../issues-done/000-NET-0045-secrets-per-program-in-keystore.done), [351-NET-0006](351-NET-0006-device-key-sealed-by-a-tpm.md) (the TPM's seal).
