# 315 — Memory across machines (later; an intention only)

**Type:** main task · **Owner:** `STO` track · **Priority:** P3 · **Status:** open · **Blocked by:** [309](309-copies-scrub-and-repair.md), [313](313-persistent-actors.md) · **Roadmap:** track B "memory across machines" · **Constitution:** MC-4.5, MC-6.12

From the kernel session's draft (`STO/2026-10-25T1557Z-KRN-draft-plan-the-store-as-a-minds-memory.md` on `internal-dev-messages`), taken by the storage session at the maintainer's word (2026-10-27). Everything here is plan, not a guarantee (MC-12.3).

## Problem

A Mind may run on several machines. Its memory then lives in several stores, which must agree without a silent loss or a silent merge.

## Plan

This is an intention, not a plan to build now:

- one Mind's stores on several machines, synchronized over MIND Link or the network with TLS;
- under a partition, a lost connection does not prove a root is gone (MC-4.5);
- conflicts reported, never resolved silently.

## Acceptance criteria

Written when the task is planned.

## Related

[309](309-copies-scrub-and-repair.md), [313](313-persistent-actors.md).
