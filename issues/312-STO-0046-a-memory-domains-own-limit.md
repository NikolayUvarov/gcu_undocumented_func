# 312-STO-0046 — A memory domain's own limit, set when it is made

**Type:** service (storage), interface · **Owner:** `STO` track · **Priority:** P2 · **Status:** in progress (2026-11-12) · **Blocked by:** — · **Main task:** [312](312-the-minds-memory.md) · **Roadmap:** track B "assertions and derived indexes" · **Constitution:** MC-4.8, MC-4.11, MC-11.5

## Problem

312's plan: "each domain has an owner, a term and a quota (MC-4.11)". MC-4.11 asks that retention have a party responsible for accounting, a term or termination condition, and a defined link to a quota, and that "copying a read right does not create unbounded unaccounted retention".

**What a memory domain has today** (308-STO-0023):
- **An owner:** the badge that made it.
- **A termination condition:** `domain-forget`, which destroys its key.
- **No quota of its own.** Its names and pins count against the owner's quota, with the volume's and every other domain's.

So a program the owner lends one domain's client, an agent with its memory, can fill the owner's whole quota through that one client. Nothing bounds what one domain retains.

**The term** is left out here. A term the store keeps by itself needs a calendar clock in `blockstore`, which holds no `rtc` client. The termination condition MC-4.11 accepts in its place is there: the owner's `domain-forget`.

## Plan

1. **A limit per domain, set when it is made:** `domain-create-limited(kib)`, in KiB; 0 is no limit of its own.
   - It is kept in the domain's key record, in the four bytes that were zero (layout 4 stays: a record with zeros there reads as before, no limit).
   - It does not change while the domain lives: a domain with another limit is a new domain.
2. **Within it:**
   - a `publish`, `commit` or `pin` through the domain's client is `quota` when the roots the domain's names and pins retain would pass its limit, each root counted once;
   - the owner's quota, and the updater's reserve (351-STO-0043), hold as before.
3. **`domain-usage(id)`:** what the domain retains, its limit (`quota`, 0 for none), and its names and pins. It is the owner's, through a client of the volume, as `domains` is.
4. **`blockstore.wit` 2.7** (minor: two new calls, no earlier one changed).
5. **`blocks domain new [kib]`** and **`blocks domain usage <id>`**.

## Acceptance criteria

- **Host tests:**
  - a domain's limit refuses a publish, a commit and a pin past it, and its usage says so;
  - the volume and another domain of the same owner are unaffected;
  - the limit is found again after a remount;
  - 0 is no limit;
  - a root counted once however many of the domain's names retain it.
- **QEMU,** where domains are made (aarch64's `store` suite, with RNDR; x86's `boot` suite store disk, with RDRAND): a limited domain refuses a publish past its limit, and after a reboot on x86 its limit and usage are the same.
- **`docs/storage`** ("Memory domains") and `docs/idl`.

## Related

[312](312-the-minds-memory.md), [308-STO-0023](../issues-done/308-STO-0023-memory-domains.done), [351-STO-0043](../issues-done/351-STO-0043-a-reserve-for-the-updaters-releases.done).
