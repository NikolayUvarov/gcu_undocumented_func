# 312-STO-0050 — A memory domain's term

**Type:** service (storage), design · **Owner:** `STO` track · **Priority:** P3 · **Status:** open · **Blocked by:** a design choice (below), then a calendar clock for `blockstore` (to be asked of `KRN` once it is made) · **Main task:** [312](../issues-done/312-the-minds-memory.done) · **Roadmap:** track B · **Constitution:** MC-4.11, Appendix B.3

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

## Plan, once chosen

- The term in the key record, beside the limit (`domain-create-limited`'s form, `blockstore.wit` minor).
- `blockstore` given a calendar clock by `init` (a `KRN` request).
- Host tests across a term's end, and a QEMU check across a reboot.

## Acceptance criteria

- What the choice says, done at a term's end and across a reboot.
- `docs/storage` ("Memory domains").

## Related

[312](../issues-done/312-the-minds-memory.done), [312-STO-0046](../issues-done/312-STO-0046-a-memory-domains-own-limit.done), [308-STO-0023](../issues-done/308-STO-0023-memory-domains.done).
