# 308-STO-0023 — Memory domains: a key per domain, and forgetting by destroying it

**Type:** storage (sealing at rest) · **Owner:** `STO` track · **Priority:** P2 · **Status:** open · **Blocked by:** [308-KRN-0184](../issues-done/308-KRN-0184-blockstore-badges-its-own-clients.done) (the store's endpoint with the badge right, from `init`), for the platform checks · **Main task:** [308](308-encryption-at-rest.md) · **Constitution:** MC-4.7, MC-4.8, MC-4.11

Split from [308](308-encryption-at-rest.md) (2026-10-28).

## Problem

With one volume key, nothing can be forgotten for sure: deleting a name leaves copies of its blocks until they are collected, and a copy of the medium keeps them. [308](308-encryption-at-rest.md) chose a key per memory domain, so a domain can be forgotten by destroying its key (MC-4.8).

## Plan

- **A domain** is a set of names and the blocks they retain, held by an owner (a badge; 312's journal and agents later). Its key is random and wrapped by the volume key, in a record of its own.
- **A record's frame names its domain.** Its seal uses that domain's key.
- **Deduplication stays within a domain:** the same bytes in two domains are two records. Otherwise one domain's forgetting would leave its blocks readable through the other's.
- **Forgetting a domain:**
  - its key record is replaced by a tombstone, and the key is gone from memory;
  - its records no longer open, and a collection frees them;
  - copies handed out before are outside this (MC-4.8), and that is said.
- **Rights:** a client works in the domains its badge holds (MC-4.7). An interface change: `blockstore.wit` 3.0 or a minor version, as the design needs.

## Decisions while building (2026-10-29)

- **Layout 4,** with layout 3 refused by name: a build of layout 3 would read a domain's records as corrupt and free them.
- **A key record** per domain (`MIND-KEY`: domain, state, owner, key), sealed under the volume key, its frame's domain 0xFFFF. A domain's records may lie before its key record. A mount scans once, and up to 64 such records wait to be opened at its end; past that it scans again, every key known. (A first version read the key records in a pass of their own, reading the medium twice. The aarch64 `vfs` suite's restart of the store on a disk holding a model then outran the command's wait.)
- **Forgetting** writes a tombstone (the key record with state 2 and no key), then erases the key record. A mount that finds both erases the key record. A collection frees the domain's records, then the tombstone. A damaged key record leaves its domain unknown: its records are kept and counted corrupt, never freed.
- **The interface is a minor version, `blockstore.wit` 2.2:** `domain-create`, `domain-client`, `domain-forget`, `domains` and the error `no-random`. Every earlier call keeps its form and works in the volume's domain, or in the domain of the client it comes through.
  - A domain's client is minted by `blockstore` from its own endpoint, with a badge from 0x100 up and the asking client's rights; it ends when the domain is forgotten.
  - So no client of another track changes (`fm` builds as it is). `init` only adds the badge right to the store's own endpoint, which minting needs (requests-KRN.md, 2026-10-29).
- **The owner** is the badge, as for quotas: a domain is its creator's, and only that owner lends or forgets it.
- **No domain without a random source.** A key derived from anything kept could not be destroyed. x86's qemu64 has no RDRAND, so the x86 `store` suite checks the refusal; the domains run in the aarch64 `store` suite (RNDR) and in the x86 store-disk check started with `+rdrand`.

## Acceptance criteria

- Host tests:
  - two domains' records open only under their own keys;
  - a forgotten domain's records are refused and then collected;
  - another domain is unaffected.
- QEMU: a domain forgotten across a reboot stays unreadable.

## Related

[308](308-encryption-at-rest.md), [303](../issues-done/303-retention-and-collection.done), [312](312-the-minds-memory.md).
