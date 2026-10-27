# 310 — Scale: the index on the medium, larger media, longer histories

**Type:** main task · **Owner:** `STO` track · **Priority:** P3 · **Status:** open · **Blocked by:** — (when sizes ask for it; before [312](312-the-minds-memory.md)'s indexes grow large) · **Roadmap:** track B "scale" · **Constitution:** MC-4.4, MC-4.5, MC-4.10

From the kernel session's draft (`STO/2026-10-25T1557Z-KRN-draft-plan-the-store-as-a-minds-memory.md` on `internal-dev-messages`), taken by the storage session at the maintainer's word (2026-10-27). Everything here is plan, not a guarantee (MC-12.3).

## Problem

docs/storage/README.md lists the limits: the whole index in memory, at most 2^32 sectors (2 TiB), 4 versions a name, 8 names a commit, no snapshot reads across requests. A mount scans the whole medium.

## Plan

- **The index on the medium** (sorted runs or a B-tree of CID to sector) with a cache in memory, and its own digest, so a mount reads it and scans only the records after it.
- **64-bit sector numbers:** layout 3, with layout 2 refused. Before the first release a new layout or interface replaces the old one, which is refused by name; no conversion code (AGENTS.md section 3, the maintainer's decision of 2026-10-25).
- **Names:** a history's length per name within a quota; commits of more than 8 names; a read session that holds one commit's point across requests.

## Acceptance criteria

- Host: 10^7 blocks; a mount's time bounded by the tail it scans.
- QEMU: a large sparse disk, its size stated.

## Related

[251-STO-0013](../issues-done/251-STO-0013-an-index-that-grows-with-the-medium.done), [304](../issues-done/304-several-names-at-once.done), [312](312-the-minds-memory.md).
