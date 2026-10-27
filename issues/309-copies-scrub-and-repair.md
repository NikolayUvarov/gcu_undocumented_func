# 309 — Copies, scrub and repair

**Type:** main task · **Owner:** `STO` track · **Priority:** P2 · **Status:** open · **Blocked by:** [307](307-the-stores-own-partition.md) · **Roadmap:** track B "scrub/repair" · **Constitution:** MC-4.4, MC-4.8, MC-5.1, MC-6.8

From the kernel session's draft (`STO/2026-10-25T1557Z-KRN-draft-plan-the-store-as-a-minds-memory.md` on `internal-dev-messages`), taken by the storage session at the maintainer's word (2026-10-27). Everything here is plan, not a guarantee (MC-12.3).

## Problem

One copy on one medium: a damaged block is found when it is read (300-STO-0005) but cannot be repaired, and a lost medium loses the store.

## Plan

- **Scrub:** a pass that reads and checks every record within a CPU and I/O budget (MC-5.1), its results in the log and in `stat`.
- **Copies:**
  - a second store on another medium, and a policy per name of which names need two;
  - `copied` ([307](307-the-stores-own-partition.md)) reached only when both media have flushed;
  - a damaged block repaired from the other copy.
- **Independence stated:** two partitions of one disk are not independent copies, and the store says so.
- **Damage beyond a block:** a store that cannot be mounted rebuilt from its copy, with a tool that checks and says what it lost.

## Acceptance criteria

- `storefaults` extended: damage in one copy found by the scrub and repaired from the other; a lost medium rebuilt.
- The random model on the host covers copies.

## Related

[300-STO-0005](../issues-done/300-STO-0005-corruption-on-the-platform.done), [307](307-the-stores-own-partition.md), [313](313-persistent-actors.md).
