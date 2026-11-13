# 312-STO-0050 — A memory domain's term

**Type:** service (storage), design · **Owner:** `STO` track · **Priority:** P2 · **Status:** in progress (2026-11-13) · **Blocked by:** a calendar clock for `blockstore`, an `rtc` client that only reads ([requests-KRN.md](requests-KRN.md), 2026-11-13), for its effect on the platform; the store's logic and its host tests need none · **Main task:** [312](../issues-done/312-the-minds-memory.done) · **Roadmap:** track B · **Constitution:** MC-4.11, Appendix B.3

Split from [312](../issues-done/312-the-minds-memory.done), whose criteria were met without it.

## Problem

Appendix B.3: "Retention is formalized as a registered obligation with a quota, term and owner."

**What a memory domain has:**
- an owner (308-STO-0023);
- a quota of its own (312-STO-0046);
- a termination condition, the owner's `domain-forget`, which MC-4.11 accepts in a term's place ("a term or termination condition").

**What it lacks** is a term the store keeps by itself. `blockstore` holds no calendar clock (`rtc`), only the monotonic time of its boot.

## The choice to make first

When a term ends, the store could:
1. **forget the domain itself.** Its key is destroyed, so nothing is readable again. A clock set wrong would destroy memory, so this needs a clock whose time the store trusts, and a margin;
2. **stop retaining it.** Names and pins stop counting, and a collection frees the records, while the key and anything still leased stay. Reading continues until then;
3. **only report it** to the owner (`domain-usage`), and leave the forgetting to the owner, as today.

Which one is the maintainer's or the owner's to say. The second and third need no key destroyed by a clock.

## Decision (2026-11-13, the maintainer)

All three ways, as a flag of each domain:

> «давай это сделаем флагом с тремя состояниями ... тогда в случае плохих часов можно будет подстраховаться и перейти в режим 3. А если требуется повышенная безопасность - то в режим 2 или 1 - когда это прямо критически важно»

- **A term and a mode per domain,** set by its owner at any time, either way. The term is a calendar time; none is no term. The modes:
  - **forget:** past its term, the store forgets the domain as `domain-forget` does, its key destroyed;
  - **release:** past its term, its names and pins retain nothing, so a collection frees its records. Writes through its client are refused (`read-only`), and its key stays until the owner forgets it;
  - **report:** past its term, the store says so (`domain-term`, and the log once), and nothing else changes.
- **Report is the default,** and the safeguard: with a clock not to be trusted, the owner sets its domains to report. Release or forget is for when it is critical.
- **Without a calendar clock** the store acts on no term. Every domain then acts as in report, and the log says why.

## Plan

- **A term record of its own:** the domain, its term, its mode and a counter, sealed under the volume key. No key material is in it, so forgetting still erases the domain's one key record. A change writes a new one and erases the one before; a mount takes the latest.
- **`blockstore.wit` minor:**
  - `domain-set-term(id, until, mode)`, the owner's, through a client of the volume;
  - `domain-term(id)`: the term, the mode, and whether it has passed.
- **`blockstore` reads the calendar time** from `rtc`, at most once a minute, and acts on the terms past it.
- **`blocks domain term <id> [<until> <mode>]`.**
- **Host tests** with a simulated clock across a term's end, each mode, a mount between, a change of term and mode; a **QEMU check** across a reboot once `blockstore` has the clock.

## Acceptance criteria

- What the choice says, done at a term's end and across a reboot.
- `docs/storage` ("Memory domains").

## Related

[312](../issues-done/312-the-minds-memory.done), [312-STO-0046](../issues-done/312-STO-0046-a-memory-domains-own-limit.done), [308-STO-0023](../issues-done/308-STO-0023-memory-domains.done).
