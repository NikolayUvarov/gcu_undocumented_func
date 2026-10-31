# 312-STO-0038 — Assertions about objects: a claim with its type, source and interpretation, kept apart from content

**Type:** storage (`libmind`, `blocks`) · **Owner:** `STO` track · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [312](312-the-minds-memory.md) · **Constitution:** MC-4.5, MC-4.6, MC-4.12, MC-4.13, Appendix B.3

Split from [312](312-the-minds-memory.md) (2026-10-31), its third step: "assertions, apart from content (MC-4.12): a claim about an object, its subject's CID, its author and its origin; signed where it crosses a trust zone (B.3); each kind of metadata names its source of truth".

No personal data is written before [308](308-encryption-at-rest.md) is done: the tests and the pilot assert test text only.

## Problem

MC-4.6: "Semantic links MUST contain a type, provenance and an interpretation version. A model's inference, an embedding and a tag are assertions by a specific source, not an immutable truth about the data." MC-4.12 keeps content identity and semantic assertions distinct, with a source of truth for each kind of metadata.

The store keeps objects, names and journals ([312-STO-0034](../issues-done/312-STO-0034-a-journal-checked-from-its-head.done)), and nothing that says something *about* an object. A tag or a title written into an object's content changes its CID, so it is no longer the same object, and nothing tells who said it.

## Plan

- **An assertion** is one canonical DAG-CBOR node of a schema of its own, `mind::assertion` (`"v": 1`; another is refused by its number, MC-4.13):
  - `subject`: the CID it is about;
  - `type`: what is said, a label;
  - `interp`: the version of that type's meaning (MC-4.6);
  - `value`: a text of at most a bound named in docs/storage, or a CID;
  - `basis`: none, or the CID of what it was derived from (a model, an index, another object);
  - `retracts`: none, or an earlier assertion of the same source it withdraws.
- **A mention, not a retention** (MC-4.5): `subject`, `basis` and `retracts` are kept as the CIDs' bytes, not as links. A walk of the store does not follow them, so an assertion never retains its subject, and a subject collected leaves its assertions in place.
- **Kept in a journal:** an assertion is the content of a journal entry of kind `assertion`, appended by `journal::append`. The journal gives it its order, its time and its `source`, the writer's label as the journal records it. So an assertion's provenance is its journal's entry, checked from the head as every entry is.
- **Retraction:** a later assertion by the same `source` that names an earlier one in `retracts` withdraws it. One by another source does not: it is that source's own claim, and both stand.
- **Reading** (`assertion::about`): the assertions of a journal about one subject, newest first, without the retracted ones, each with its entry's sequence and source. The journal is checked whole from its head, and each assertion read and checked against its CID. A scan first: an index by subject waits for the sizes to ask for it, as the text index did ([312-STO-0035](../issues-done/312-STO-0035-a-text-index-derived-from-a-journal.done)).
- **The sources of truth** (MC-4.12), named in docs/storage:
  - an assertion with no `basis` is its source's word, and the journal is its record;
  - one with a `basis` is derived, may be dropped, and is made again from its basis.
- **The pilot, `blocks assert`:**
  - `add <journal> <cid> <type> <text>` and `add <journal> <cid> <type> -c <cid>`, printing the entry's sequence and the assertion's CID;
  - `about <journal> <cid>`, the assertions newest first;
  - `retract <journal> <assertion>`;
  - with `-d <domain>`, as every command of the tool.
- **Not on the wire:** `blockstore.wit` and the store's layout do not change.

## What it does not give, said in docs/storage

- **Not signed.** B.3 asks for a signature where provenance crosses a trust zone; a local record needs none. The source is the writer's label, as a journal's is.
- **Not true:** an assertion says who claims what, not that it holds (MC-4.6).
- **No rights:** an assertion lives in its journal's store and domain, and reading one about an object grants nothing on the object (MC-4.6).
- **A scan:** each `about` reads the whole journal.

## Acceptance criteria

- **Host,** `tests/assertion_host.rs`, over the block store's logic on a simulated medium:
  - an assertion's one encoding, its value a text or a CID, with a basis or not; its refusals: every truncation, a trailing byte, a value past its bound, a label out of range, another schema version refused by its number;
  - `about` answers the assertions of one subject newest first, and none of another subject;
  - a retraction by the same source withdraws, and one by another source does not; retracting what is not an earlier assertion of the journal is refused;
  - the subject collected while assertions mention it: the assertions stay, and `about` still answers them;
  - an assertion's block changed on the medium is refused when read, and a journal that does not check answers nothing.
- **QEMU `store` suite (x86, aarch64):**
  - `blocks assert add` twice about an object, `about` lists both newest first;
  - `retract` withdraws one, and `about` lists the other;
  - the object unpublished and collected: `about` still lists the assertion.
- **docs/storage** (a section on assertions, with what they do not give) and the profile's Article 4 rows.

## Related

[312](312-the-minds-memory.md), [312-STO-0034](../issues-done/312-STO-0034-a-journal-checked-from-its-head.done), [312-STO-0035](../issues-done/312-STO-0035-a-text-index-derived-from-a-journal.done), [303](../issues-done/303-retention-and-collection.done), [308](308-encryption-at-rest.md).
