# 314 — Models and large objects mapped from the store

**Type:** main task · **Owner:** `STO` track · **Priority:** P3 · **Status:** open · **Blocked by:** [307](307-the-stores-own-partition.md) · **Roadmap:** track B "models in the store" · **Constitution:** MC-4.4, MC-5.1

From the kernel session's draft (`STO/2026-10-25T1557Z-KRN-draft-plan-the-store-as-a-minds-memory.md` on `internal-dev-messages`), taken by the storage session at the maintainer's word (2026-10-27). Everything here is plan, not a guarantee (MC-12.3).

## Problem

Speech models are objects in the store (251-STO-0010), but a model is read into a program's memory by copying. [251-STO-0015](251-STO-0015-a-model-read-into-memory.md) reads one into memory; `NRL`'s runtime needs to map large models read-only without a second copy.

## Plan

- 251-STO-0015 widened: a model read once into a memory object that `NRL`'s runtime maps read-only, checked against its CID, without a second copy.

## Acceptance criteria

- QEMU: a model's object mapped and checked, with its size and the time it took stated.

## Related

[251-STO-0015](251-STO-0015-a-model-read-into-memory.md), [251-STO-0010](../issues-done/251-STO-0010-speech-models-in-the-store.done), 262-KRN-0162 (`vfs.wit` 2.4 `fill`), 900 (`NRL`).
