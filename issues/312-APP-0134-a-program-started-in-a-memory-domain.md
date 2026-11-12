# 312-APP-0134 — The shell starts a program in a memory domain

**Type:** tools (`shell`) · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open (`run -d` done, 2026-11-12; its QEMU check on aarch64 to pass) · **Blocked by:** — · **Main task:** [312](312-the-minds-memory.md) · **Roadmap:** track G · **Constitution:** MC-3.11, MC-11.11

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

## Progress (2026-11-12)

- **`run -d <domain> <program> [arguments] [&]`** (`shell/src/domainrun.rs`, `shell/src/main.rs`'s `start_with`): the store's client of the domain is asked for through the client the program would have had (`SLOT_BLOCKSTORE` for `REQUEST_BLOCKSTORE`, `SLOT_BLOCKSTORE_READ` for `REQUEST_BLOCKSTORE_READ`), after the other grants, and lent in `SLOT_BLOCKSTORE` in place of the volume's; the serial line says `[SHELL] BLOCKS STARTS IN MEMORY DOMAIN 2, READ AND WRITE`.
- **Refused before the start,** each with its text: `-d` with no number or no program (`USAGE: RUN [-D <DOMAIN>] <NAME> [ARGUMENTS] [&]`), a program that asks for no block store, one with no store client to lend (no store, or a script without `requires: blockstore`), a service; after the loader's session begins, the store's `not-found`, `rights` and `full`, the session aborted.
- **Tested:** `tests/shell_host.rs`'s `run_d_names_a_memory_domain_before_the_program` (the words and the texts). **Checked by** the `store` suite's `run_in_domain_check`: the refusals on x86, where no domain is made without a random source; and on aarch64, `blocks` started in the first domain reading that domain's name, not seen from the second, and refused `domain new` and `domains` through the domain's client.
- **A read-only start:** the `store` suite's `blocksro` (`blocks` asking for `REQUEST_BLOCKSTORE_READ` alone) started with `-d` reads the domain's name and is refused a put and a publish: the domain's client has the get badge's rights (`blocksro` added after STO's answer of 2026-11-12, which leaves this check to APP).

## Related

[312](312-the-minds-memory.md), [requests-APP.md](requests-APP.md), 308-STO-0023 (memory domains).
