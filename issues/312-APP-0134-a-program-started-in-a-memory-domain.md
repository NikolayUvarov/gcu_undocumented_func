# 312-APP-0134 — The shell starts a program in a memory domain

**Type:** tools (`shell`) · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open (`run -d` done, 2026-11-12; a read-only start waits for the store) · **Blocked by:** — (a read-only start: 312-STO-0049, `blockstore.wit` 2.8 `domain-client-with(id, rights)`, STO's answer of 2026-11-12) · **Main task:** [312](312-the-minds-memory.md) · **Roadmap:** track G · **Constitution:** MC-3.11, MC-11.11

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

- **`run -d <domain> <program> [arguments] [&]`** (`shell/src/domainrun.rs`, `shell/src/main.rs`'s `start_with`): the store's client of the domain is asked for through the shell's client (`SLOT_BLOCKSTORE`), after the other grants, and lent in `SLOT_BLOCKSTORE` in place of the volume's; the system log says `[SHELL] BLOCKS STARTS IN MEMORY DOMAIN 2`.
- **Refused before the start,** each with its text: `-d` with no number or no program (`USAGE: RUN [-D <DOMAIN>] <NAME> [ARGUMENTS] [&]`), a program that asks for no block store, one with no store client to lend (no store, or a script without `requires: blockstore`), a service, and for now one that asks only to read; after the loader's session begins, the store's `not-found`, `rights` and `full`, the session aborted.
- **Tested:** `tests/shell_host.rs`'s `run_d_names_a_memory_domain_before_the_program` (the words and the texts). The `store` suite's `run_in_domain_check` passes: on x86 (31 checks) the refusals, where no domain is made without a random source; on aarch64 (32 checks, on 34635ebe52) `blocks` started in the first domain reading that domain's name, not seen from the second, refused `domain new` and `domains` through the domain's client, and `blocksro` refused the start.
- **A read-only start, not yet:** the aarch64 `store` suite showed that the store refuses `domain-client` through the shell's read-only client (`rights`): it mints a domain's client with the rights and owner of the client it is asked through, the owner being the whole badge (`OWNER_BITS`), and the shell's domains are its writing client's. A client of the writing one would give a program that only reads the right to write, so the shell refuses such a start with its own text (`RUN -D: NOT YET FOR A PROGRAM THAT ONLY READS`, checked with `blocksro`), and asked STO for a domain's client with fewer rights than the caller's: 312-STO-0049, `domain-client-with(id, rights)`, asked through the writing client with `rights` = `BADGE_GET`. The shell takes it once it is in `main`.

## Related

[312](312-the-minds-memory.md), [requests-APP.md](requests-APP.md), 308-STO-0023 (memory domains).
