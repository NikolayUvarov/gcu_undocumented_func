# 150 — User memory beyond the kernel arena

**Type:** kernel · **Owner:** kernel track · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Roadmap:** stage II follow-up, track G (voice V3) · **Constitution:** MC-5.4, MC-3.13

## Problem

Task memory (images, stacks, screens, program heaps, memory objects) comes from the 64 MiB kernel arena, while a QEMU machine has 512 MiB and real ones far more. A program heap is limited to 16 MiB in 32 blocks and all memory objects to 16 MiB together. Voice V3 needs speech models of 40–80 MB, read-only and best shared between tasks ([docs/voice](../docs/voice/README.md) §4); larger tools and files hit the same wall.

## Plan

- A physical frame allocator over the free RAM of the firmware map (outside the arena, the kernel image, boot images and the framebuffer); task pages (heap blocks, memory objects, screens) come from it, kernel structures stay in the arena.
- Per-task memory quota delegated at `SPAWN` like the task and endpoint quotas (MC-3.13); `HEAP_MAX_BYTES`/`HEAP_MAX_BLOCKS` become defaults of that quota.
- Large memory objects: a sealed read-only object of up to the quota, mappable by several tasks (a model loaded once).
- `STAT_MEMORY` reports free frames and per-task use; tests for exhaustion, rollback and reclamation as for the arena today.

## Acceptance criteria

- A program allocates and frees 128 MiB in a 512 MiB VM; a 64 MiB sealed object is mapped read-only by two tasks; the `memory`, `heap` and `isolation` suites pass.

## Related

[docs/voice](../docs/voice/README.md) (V3), [075](../issues-done/075-stat-fields-for-the-monitors.done).
