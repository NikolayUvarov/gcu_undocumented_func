# 032 — Program heap `mind::alloc`

**Type:** feature · **Priority:** P0 · **Status:** open · **Blocked by:** — · **Roadmap:** track G, tools plan F1 · **Constitution:** MC-5.1

## Problem

Programs get memory only as page blocks (`ALLOC`/`FREE`, `mind::mem::Pages`, 32 blocks and 16 MiB per task). There is no allocator for smaller objects, so programs cannot use `alloc` (`Vec`, `String`, `Box`). The editor, the file manager and the observation tools need dynamic collections ([docs/tools](../docs/tools/README.md) F1).

## Plan

- `libmind/src/heap.rs`, enabled by the cargo feature `alloc` of `libmind`: a `GlobalAlloc` over the page-block system calls.
  - Small objects (up to 2 KiB) in power-of-two size classes, carved from 4 KiB pages, one free list per class.
  - Medium objects (up to 256 KiB) as runs of pages inside 1 MiB arenas, first fit over a page bitmap.
  - Large objects as their own page block.
  - Arenas are added on demand, so a task never uses more than a few of its 32 blocks for small objects.
- The core is generic over a page source and builds on the host; `tests/heap_host.rs` checks alignment, reuse, no overlap and bounds under random workloads.
- Allocation failure returns null; `alloc_error_handler` is not available on stable no_std, so `handle_alloc_error` ends in the panic handler, which logs and exits the task.
- Services do not enable the feature (their memory stays static).

## Acceptance criteria

- A program with `features = ["alloc"]` uses `Vec`/`String`; host tests pass; existing programs build unchanged.

## Related

[docs/tools](../docs/tools/README.md) F1; README "Private program heap".
