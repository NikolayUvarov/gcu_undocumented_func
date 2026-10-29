# 308-KRN-0184 — `blockstore` holds its endpoint with the badge right, to mint its domains' clients

**Type:** `init` (grants) · **Owner:** `KRN` · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Main task:** [308](308-encryption-at-rest.md) (STO's), for [308-STO-0023](308-STO-0023-memory-domains.md) · **Constitution:** MC-3.1, MC-3.4, MC-12.4

**Asked by:** the storage track (`STO`) in `issues/requests-KRN.md`, 2026-10-29, "The block store's own endpoint with the badge right (STO, for 308-STO-0023)".

## Problem

- **A memory domain is worked in through a client the store mints for it** from its own endpoint, with a badge from `0x100` up (`blockstore.wit` 2.2 `domain-client`), as `vfs_server` mints confined file clients and `keystore` programs' secrets clients.
- **`init` grants `blockstore` its endpoint in `SLOT_SERVICE` with `ALL` and without `CAP_BADGE`** (`init/src/main.rs`, the `"blockstore"` arm), so the mint is refused: in QEMU a domain is made, and then `blocks -d 1 put` gets no client.

## Plan

- **`init`:** the store's endpoint with `ALL | CAP_BADGE`, as `vfs_server`'s.
- **The badges `init` gives the store's clients stay below `0x100`** (`mind::blockstore::FIRST_DOMAIN_CLIENT`). The store treats a badge from there up that it did not mint as one with no right; that is the storage track's side.
- **Docs:** `docs/profile/bootstrap.md`'s row for `blockstore` says it may badge its own clients.

## Acceptance criteria

- **Builds** x86 and aarch64; the `services` and `store` suites pass with the grant.
- **The storage track's checks of domains pass** once its branch lands (`claude/relaxed-meitner-5bmhpz`): the `store` suite on aarch64, where RNDR gives the key, and the `boot` suite's store disk on x86 with `+rdrand`, a domain forgotten across a reboot.

## Related

[308-STO-0023](308-STO-0023-memory-domains.md), [308-KRN-0183](308-KRN-0183-a-secrets-client-for-blockstore.md) (the store's other grant from `init`).
