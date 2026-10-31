# 312-STO-0035 — A text index derived from a journal: rebuilt equal to itself, a stale one told, every answer checked

**Type:** storage (`libmind`, `blocks`) · **Owner:** `STO` track · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [312](312-the-minds-memory.md) · **Constitution:** MC-4.5, MC-4.6, MC-4.12, MC-4.13, MC-10.3

Split from [312](312-the-minds-memory.md) (2026-10-31), its second step: "derived indexes: full text … an exact scan first … an index is never authority or proof that a version is current, and may be dropped and rebuilt (MC-4.12)". Vectors wait for `NRL`'s embeddings; assertions are a later task of 312.

No personal data is written before [308](308-encryption-at-rest.md) is done: the tests and the pilot index test text only.

## Problem

A journal ([312-STO-0034](../issues-done/312-STO-0034-a-journal-checked-from-its-head.done)) is read from its head, newest first. Finding the entries whose content holds a word means reading every entry and every content. An index answers sooner, but MC-4.12 and MC-4.6 bound it:
- it is derived: it can be dropped and rebuilt, and a rebuild gives the same index;
- it is not proof that it is current: a reader tells a stale one, and by how much;
- it is not authority: each answer is checked against the journal, and an index that answers wrong is refused.

## Plan

- **An index is an object of `tree`** ([311-STO-0027](../issues-done/311-STO-0027-directories-as-objects.done)), published under a name of its own by compare-and-swap, `index/<journal>` unless one is given. So the store retains it, `mind::car` copies it, and the owners' sizes count it, with nothing new in the store and nothing on the wire.
  - `manifest`, a file: a canonical DAG-CBOR map of a schema of its own, `mind::index`, versioned (`"v"`; another is refused by its number, MC-4.13). It names the index's kind (`"text"`), the tokenizer's version, the journal's name, and the head's sequence and CID it was built from, with the counts of entries, terms and postings.
  - the terms, in directories sharded by their first bytes, each term a file of its postings: the sequences of the entries whose content holds it, ascending, in one encoding.
  - Built only from what the journal holds, in one order: the same head gives the same bytes, so the same root's CID.
- **The tokenizer, version 1:** a term is a run of ASCII letters and digits in a content, lowercased, 1 to 32 bytes; a longer run is cut into pieces of 32, and every other byte separates. Text beyond ASCII is not indexed; docs/storage says so.
- **Building** (`index::build`): the journal checked from its head first (`journal::check`): no index is built from a journal that does not check. Then each entry's content read whole, every block checked against its CID, and tokenized. `libmind` has no heap: the terms and postings are gathered in a work area the caller gives, and a journal past it is refused (`full`), nothing published. The bounds are said in docs/storage.
- **Stale or current** (`index::state`): an index is current when its head is the journal's head. It is stale when the journal checks and the entry at the index's sequence is the index's head: the journal grew by the entries above it. Anything else is refused: the journal does not check, or its entry at that sequence is another, or the journal is shorter (an index of another journal, or of one rewritten).
- **Search** (`index::search`): the postings of a word's terms, then each posting checked against the journal: its entry read and its content tokenized again, so a posting whose entry does not hold the term is a false answer, and the index is refused. A stale index answers for the entries up to its sequence, and the entries above it are scanned. So an answer is always the exact scan's.
- **The exact scan** (`index::scan`) is the answer without an index: every entry's content tokenized. The host tests hold every search to it.
- **The check** (`index::verify`): the index rebuilt in the work area and its root compared with the published one: equal, stale (rebuilt from the index's own head, equal), or differs, which also tells a posting dropped, which a search cannot.
- **The pilot, `blocks index`:**
  - `build <journal> [<name>]`, which prints the counts and the root;
  - `search <journal> <word> [<name>]`, the sequences, newest first, and whether the index is current or how many entries it lacks;
  - `check <journal> [<name>]`, which prints `equal`, `stale <n>` or `differs`;
  - with `-d <domain>`, as every command of the tool.
- **Not on the wire:** `blockstore.wit` and the store's layout do not change. An index is a client's use of `put`, `publish`, `snapshot` and `get`.

## What it does not give, said in docs/storage

- **Not vectors,** and no term beyond ASCII.
- **Bounded:** by the work area, so a journal past it has no index. An index on the medium that grows is [310](310-the-index-on-the-medium.md)'s.
- **Not faster to check:** every answer is checked, and the check reads the journal down to the oldest entry answered. An index saves the tokenizing of the entries no term names; it does not make a journal's check shorter.
- **No rights:** an index lives in the store and domain its journal does; it grants nothing and reads nothing its reader's client cannot (MC-4.6).

## Acceptance criteria

- **Host,** `tests/index_host.rs`, over the block store's logic on a simulated medium:
  - an index built twice from the same journal of at least 200 entries (words from a seeded generator): the same root, also after a remount and a collection;
  - every search equal to the exact scan, for each word the journal holds and for words it does not;
  - after appends, the index told stale by how many entries, and searches still equal to the scan;
  - refused: an index of another journal; an index of a journal rewritten from an older point; a journal that does not check;
  - an index changed by hand and published under its name: a posting added is a false answer, refused by the search; a posting dropped is told by `verify`, `differs`;
  - a manifest of another version or kind refused by its number;
  - a journal past the work area refused with nothing published.
- **QEMU `store` suite (x86, aarch64):**
  - `blocks journal append` several times, then `blocks index build`, and `search` prints the sequences;
  - after another append, `search` says the index lacks one entry and still finds it, and `check` says `stale 1`;
  - rebuilt, `check` says `equal`; after a collection, the same.
- **docs/storage** (a section on derived indexes, with what they do not give) and the profile's Article 4 rows.

## Related

[312](312-the-minds-memory.md), [312-STO-0034](../issues-done/312-STO-0034-a-journal-checked-from-its-head.done), [311-STO-0027](../issues-done/311-STO-0027-directories-as-objects.done), [310](310-the-index-on-the-medium.md), [303](../issues-done/303-retention-and-collection.done).
