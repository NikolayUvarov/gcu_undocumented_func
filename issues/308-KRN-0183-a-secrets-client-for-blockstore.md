# 308-KRN-0183 — A secrets client for `blockstore`, under a name no program can take

**Type:** `init` (grants), `keystore` (`idl/keystore.wit`) · **Owner:** `KRN` (with `NET`'s `keystore`, which the kernel session holds) · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Main task:** [308](308-encryption-at-rest.md) (STO's), for [308-STO-0022](308-STO-0022-the-volume-key-held-by-keystore.md) · **Constitution:** MC-3.1, MC-6.8, MC-10.3, MC-11.9, MC-12.4

**Asked by:** the storage track (`STO`) in `issues/requests-KRN.md`, 2026-10-29, "The block store's volume key kept by `keystore` (STO, for 308-STO-0022)".

## Problem

- **The store's volume key is in its superblock in the clear** ([308-STO-0021](../issues-done/308-STO-0021-sealed-records.done) sealed the records, not the key). Whoever reads the medium opens the records.
- **`keystore` already keeps a program's secrets** (`keystore.wit` 1.4, [000-NET-0045](../issues-done/000-NET-0045-secrets-per-program-in-keystore.done)), but only for a program the loader runs by its name ([000-KRN-0182](../issues-done/000-KRN-0182-a-programs-secrets-client.done)). A boot service gets no such client.
- **A program's name is not enough for a service.** The loader already refuses to run a program by a boot service's name, so no program gets a client named `blockstore`. But a service's secrets kept as a program's would share their directory and their key's label with every program's, and one mistake in the loader's rule would open them; so they need a namespace of their own.

## Plan

**Changed on 2026-10-29, before any code: a badge, not a call.** The plan first had `init` ask `keystore` for a service's client with a badge of its own (`BADGE_KEY_INIT`) and a new call. A client badged for the service does the same with less: the Wi-Fi station and the SSH service already reach their part of `keystore` that way (`BADGE_KEY_STATION`, `BADGE_KEY_SSH`), only `init` mints badged clients of `keystore`, and nothing has to be asked at boot.

- **`keystore.wit` 1.5** (no new call):
  - on a client badged `BADGE_KEY_BLOCKSTORE` (5, `mind::network`), the secrets calls of 1.4 (`secret-put`, `secret-get`, `secret-delete`, `secret-list`) are about the block store's own secrets;
  - they are kept apart, in `system/keystore/services/blockstore.kept`, sealed under a key whose label names a service (`MIND keystore: a service's secrets`), so neither a program's name nor a file copied between the two directories opens them;
  - the same budget as a program's: 16 secrets, 65536 bytes.
- **`init`:**
  - lends `blockstore` that client in its slot 3 when `keystore` runs; else it starts the store without it and logs why (MC-6.8): the store depends on `keystore`, never the other way round;
  - starts `blockstore` right after `keystore` (`services::LATE`, init's start order; `BOOT_SERVICES` and the ABI are unchanged), and `services.txt` cannot put `keystore` after it;
  - a restarted store gets a new client the same way.
- **Docs:** `idl/keystore.wit`, `docs/profile/bootstrap.md` (`blockstore`'s slot 3, `keystore`'s row), the profile's MC-11.9 row.

## Acceptance criteria

- **Host tests:**
  - a service's and a program's secrets never meet: other directories, other labels, other keys for the same name, and a program's key as 1.4 made it (`tests/secrets_host.rs`);
  - `blockstore` right after `keystore` in init's start order, the other images in `BOOT_SERVICES`' order, `after keystore blockstore` refused, the store left on when `keystore` is off (`tests/services_host.rs`).
- **QEMU** (`tls` suite, `device_key_check`, a raw image booted three times with a blank store disk):
  - `blockstore` holds `keystore`'s client badged 5 in slot 3;
  - the volume key of its disk is kept by `keystore` as the store's secret, not on the medium, and read back at the next boot, when the store mounts its disk with it;
  - no program runs by the store's name (the loader refuses it), and another program's client (`netcopy`, `netcheck` by that name) finds none of the store's secrets and does not open the store's file copied under its own name;
  - under a new device key the store's file opens no more.
- **The other suites** pass. The storage track's suites check the rest (308-STO-0022).

## Related

[308-STO-0022](308-STO-0022-the-volume-key-held-by-keystore.md), [000-KRN-0182](../issues-done/000-KRN-0182-a-programs-secrets-client.done), [000-NET-0045](../issues-done/000-NET-0045-secrets-per-program-in-keystore.done), [351-NET-0006](351-NET-0006-device-key-sealed-by-a-tpm.md) (the TPM's seal).
