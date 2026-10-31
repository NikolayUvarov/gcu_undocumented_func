# 312 — The Mind's memory: a journal, assertions and derived indexes

**Type:** main task · **Owner:** `STO` track · **Priority:** P2 · **Status:** open · **Blocked by:** [308](308-encryption-at-rest.md) (no personal data before it) · **Roadmap:** track B "assertions and derived indexes" · **Constitution:** MC-4.5, MC-4.7, MC-4.8, MC-4.11, MC-4.12, MC-10.3, MC-11.5, Appendix B.3

From the kernel session's draft (`STO/2026-10-25T1557Z-KRN-draft-plan-the-store-as-a-minds-memory.md` on `internal-dev-messages`), taken by the storage session at the maintainer's word (2026-10-27). Everything here is plan, not a guarantee (MC-12.3).

## Problem

A Mind needs memory with an explicit history, deliberate and accounted forgetting, and indexes that never become authority. The store keeps objects and names, but no journal, no assertions about objects and no indexes over them.

## Plan

- **The journal**, append-only per memory domain: each entry is an object (a monotonic time and the calendar time with its source; the source program and its identity from the badge; a kind; the content's CID; a link to the entry before). The head is published by compare-and-swap, so a reader checks order and completeness from it.
- **Assertions**, apart from content (MC-4.12): a claim about an object, its subject's CID, its author and its origin; signed where it crosses a trust zone (B.3); each kind of metadata names its source of truth.
- **Derived indexes:** full text; and vectors.
  - **Who does what** (the storage session's choice, 2026-10-27): `NRL`'s runtime computes the embeddings; this track keeps the index. Its manifest names the model's CID and the index's parameters, so it can be rebuilt and a stale one detected.
  - **The structure:** an exact scan first; an approximate index such as HNSW when the sizes ask for it, with recall measured against the exact scan.
  - An index is never authority or proof that a version is current, and may be dropped and rebuilt (MC-4.12).
- **Retention and forgetting:** each domain has an owner, a term and a quota (MC-4.11); "forget" removes the names and collects; "forget for sure" destroys the domain's key ([308](308-encryption-at-rest.md)); copies already handed over are outside both, and that is said (MC-4.8).
- **Rights and data:** a memory domain is a capability, and an agent (260) holds its own only; text read from memory is data, never an instruction (MC-11.5).

## Tasks

- [312-STO-0034](../issues-done/312-STO-0034-a-journal-checked-from-its-head.done): the journal, append-only, checked from its head; `blocks journal` as the pilot. Done 2026-10-31.

## Acceptance criteria

- Host: the journal's check from its head; an index rebuilt equal to itself; a stale index detected.
- QEMU: a stand-in agent writes and reads its memory; forgetting frees what it should; another domain refused without a grant.

## Related

[303](../issues-done/303-retention-and-collection.done), [308](308-encryption-at-rest.md), [310](310-the-index-on-the-medium.md), [313](313-persistent-actors.md), 260, 900 (`NRL`).
