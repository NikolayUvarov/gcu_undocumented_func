# 312-STO-0034 — A journal, append-only, checked from its head

**Type:** storage (`libmind`, `blockstore`, `blocks`) · **Owner:** `STO` track · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [312](312-the-minds-memory.md) · **Constitution:** MC-4.3, MC-4.5, MC-4.10, MC-4.12, MC-10.3

Split from [312](312-the-minds-memory.md) (2026-10-30), its first step: "the journal, append-only per memory domain … the head is published by compare-and-swap, so a reader checks order and completeness from it". Assertions and derived indexes are later tasks of 312.

No personal data is written before [308](308-encryption-at-rest.md) is done: the tests and the pilot write test text only.

## Problem

The store keeps objects and names, and a name's history keeps 4 versions. Nothing keeps an ordered record of what happened that a reader can check whole:
- every entry there;
- none reordered, changed or dropped;
- the record never rewritten from an older point.

## Plan

- **An entry** is a canonical DAG-CBOR node of a schema of its own, `mind::journal`, versioned (`"v"`; another is refused, MC-4.13). It holds:
  - its sequence number, from 1;
  - the time: the monotonic time and the boot it belongs to, and the calendar time with its source (none, the clock chip, the network). The calendar time is the writer's claim, and the check never orders by it;
  - the source, as the writer gives it: a program's name;
  - a kind: a short label;
  - the content's CID, a link;
  - a link to the entry before, absent for the first.
- **The store retains a journal whole.**
  - It takes an entry as a typed `dag-cbor` block, as it takes `dag`'s nodes and `tree`'s directories.
  - Its walk follows the links to the entry before one after another, never nested. `tree` bounds a path at 32 directories, and a journal is longer than that.
  - So a collection keeps every entry and every content a journal's head retains, and `dag::complete` refuses a head whose chain lacks a block.
- **Appending** puts the content and the entry, then publishes the entry under the journal's name by compare-and-swap from the version read.
  - Of two writers that read the same head, the second gets `conflict`, reads the new head and appends after it. It never forks the journal.
  - An entry's sequence number is the name's version it is published at.
- **The check from the head** reads the name's current version and root, then walks the chain to the first entry, within a bound of steps. It refuses:
  - a sequence number that is not the version, the next one down, or ending above 1;
  - a block missing or corrupt, an entry of another schema;
  - a monotonic time that goes back within one boot.

  A head published again at a later version, a rewrite, is refused this way, as a checkpoint's epoch is (313-STO-0033). So is a journal whose name was removed and published again.
- **A memory domain:** a journal in a domain is written through that domain's client (308-STO-0023), and is forgotten with it. Nothing new is needed for this; the tests show it.
- **The pilot, `blocks journal`:**
  - `append <name> <kind> <text>` and `append <name> <kind> -f <file>`;
  - `check <name>`, which prints the count, the first and last sequence numbers, and the result;
  - `list <name> [n]`, the last entries, newest first;
  - with `-d <domain>`, as every command of the tool.
- **Not on the wire:** `blockstore.wit` does not change. A journal is a client's use of `put`, `publish`, `resolve` and `get`.

## What it does not give, said in docs/storage

- **The source and the calendar time are the writer's claims.** The store records the publisher's badge in a name record, but `history` does not return it. A source attested by the store is a later step, on the wire.
- **A writer with the name's rights can append anything.** The check shows the order and completeness of what was published, not that the entries are true (MC-4.12).
- **A journal is as durable as its store** (307-STO-0019's classes). A copy (309-STO-0031) carries it whole, since a copy is of a name's object.

## Acceptance criteria

- **Host,** `tests/journal_host.rs`, over the block store's logic on a simulated medium:
  - 1000 entries appended, checked whole from the head, also after a remount and a collection;
  - the check refuses each of these:
    - an entry's block erased;
    - an entry changed (another entry with the same links);
    - an older head published again at a later version;
    - a name removed and published again;
    - a monotonic time that goes back within one boot;
  - two writers: the second refused with `conflict`, appending after re-reading, and the check passes;
  - a journal in a memory domain, gone with the domain's forgetting;
  - an append stopped after each of the store's calls: the check passes on whatever the head is.
- **QEMU `store` suite (x86, aarch64):**
  - `blocks journal append` three times;
  - `check` and `list` print them;
  - after a collection they are still there;
  - an older head published again by hand with `blocks publish` is refused by `check`.
- **docs/storage** (a section on the journal, with what it does not give) and the profile's Article 4 rows.

## Related

[312](312-the-minds-memory.md), [311-STO-0027](../issues-done/311-STO-0027-directories-as-objects.done), [308-STO-0023](../issues-done/308-STO-0023-memory-domains.done), [313-STO-0033](../issues-done/313-STO-0033-a-component-moved-through-a-copy.done), [303](../issues-done/303-retention-and-collection.done).
