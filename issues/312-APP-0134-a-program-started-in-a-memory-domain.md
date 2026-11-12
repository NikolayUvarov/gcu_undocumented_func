# 312-APP-0134 — The shell starts a program in a memory domain

**Type:** tools (`shell`) · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [312](312-the-minds-memory.md) · **Roadmap:** track G · **Constitution:** MC-3.11, MC-11.11

**Asked by:** STO, for 312's QEMU criterion, in [requests-APP.md](requests-APP.md), 2026-11-12. Numbered on 2026-11-12.

## Problem

A memory domain is a set of names, pins and blocks under a key of its own (308-STO-0023). Its owner gets a client of it from the store (`blockstore.wit`'s `domain-client`), through which a program reaches that domain alone. The shell lends a program its own client of the volume for `REQUEST_BLOCKSTORE`, so no program can be started holding one domain only, as an agent should (312).

## Plan

- **`run -d <domain> <program> [arguments] [&]`:** the program gets, in `SLOT_BLOCKSTORE` in place of the shell's client of the volume, the client of domain `<domain>` the store mints for the shell: through the shell's own client for `REQUEST_BLOCKSTORE`, through its read-only one for `REQUEST_BLOCKSTORE_READ`, so with the get badge alone.
- **Refused before the start:** a program that asks for no block store, a number that is not one, and a domain the store does not give (not the shell's, or not live), with the store's error.
- **The rest** is lent as for any start; a script's `run -d` needs `requires: blockstore`.

## Acceptance criteria

- **Host tests:** the words of `run -d`.
- **QEMU (`store` suite):** `blocks` started with `-d` on a domain the shell made writes and reads its own names, and finds neither the volume's nor another domain's; a read-only start is refused a put; an unknown domain is refused.

## Related

[312](312-the-minds-memory.md), [requests-APP.md](requests-APP.md), 308-STO-0023 (memory domains).
