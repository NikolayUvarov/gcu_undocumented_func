# 312-STO-0049 — A domain's client with fewer rights than its owner's

**Type:** service (storage), interface · **Owner:** `STO` track · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [312](312-the-minds-memory.md) · **Roadmap:** track B "assertions and derived indexes" · **Constitution:** MC-4.7, MC-4.11, MC-11.5

Asked by APP on 2026-11-12 in `requests-STO.md`, for [312-APP-0134](312-APP-0134-a-program-started-in-a-memory-domain.md)'s read-only start (`run -d` of a program that asks only for `REQUEST_BLOCKSTORE_READ`).

## Problem

- **What there is:** `blockstore.wit` 2.2's `domain-client(id)` mints a client of domain `id` with the caller's rights, for the caller's owner. The owner is the caller's whole badge (`OWNER_BITS`), so the shell's writing client and its read-only one (badge 1) are two owners.
- **What follows:** the shell makes its domains through its writing client. Its read-only client gets `rights` from `domain-client`, and its writing client can only lend a client that writes. A program that only reads therefore cannot be started in a domain; `run -d` refuses it today (312-APP-0134).
- **What MC-4.7 asks:** a right is lent no wider than the task needs.

## Plan

1. **`domain-client-with(id, rights)`,** `blockstore.wit` 2.8 (minor: one new call, `domain-client` unchanged):
   - a client of domain `id` of the caller's owner, as `domain-client` gives one;
   - its rights are those of `rights` the caller holds too (`BADGE_GET` 1, `BADGE_PUT` 2, `BADGE_PUBLISH` 4);
   - none of them in common, or a bit that is no such right: `invalid`;
   - the recovery right never: a domain's client recovers nothing.
2. **Its clients are the domain's** as `domain-client`'s are: revoked when the domain is forgotten, refused through a domain's client.
3. **`blocks -dr <id>`** runs a command through a client of the domain that may only read.

## Acceptance criteria

- **QEMU `store` suite** (aarch64, where domains are made):
  - through `blocks -dr`, the domain's name resolved and its block read;
  - its `put` and `publish` refused (`rights`), and logged;
  - the client gone with the domain's forgetting;
  - rights the caller lacks, and none, refused (`invalid`).
- **`docs/storage`** ("Memory domains") and `docs/idl`.
- **Then APP** lends that client for a read-only `run -d` (312-APP-0134).

## Related

[312](312-the-minds-memory.md), [312-APP-0134](312-APP-0134-a-program-started-in-a-memory-domain.md), [308-STO-0023](../issues-done/308-STO-0023-memory-domains.done), [300-STO-0004](../issues-done/300-STO-0004-rights-by-badge.done).
