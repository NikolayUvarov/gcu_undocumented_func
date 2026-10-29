# 308-STO-0023 — Memory domains: a key per domain, and forgetting by destroying it

**Type:** storage (sealing at rest) · **Owner:** `STO` track · **Priority:** P2 · **Status:** open · **Blocked by:** [308-KRN-0184](308-KRN-0184-blockstore-badges-its-own-clients.md) (the store's endpoint with the badge right, from `init`), for the platform checks · **Main task:** [308](308-encryption-at-rest.md) · **Constitution:** MC-4.7, MC-4.8, MC-4.11

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

## Acceptance criteria

- Host tests:
  - two domains' records open only under their own keys;
  - a forgotten domain's records are refused and then collected;
  - another domain is unaffected.
- QEMU: a domain forgotten across a reboot stays unreadable.

## Related

[308](308-encryption-at-rest.md), [303](../issues-done/303-retention-and-collection.done), [312](312-the-minds-memory.md).
