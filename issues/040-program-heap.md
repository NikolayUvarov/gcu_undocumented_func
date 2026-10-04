# 040 — Program heap `mind::alloc` (tools F1)

**Type:** tool · **Owner:** tools track · **Priority:** P1 · **Status:** open · **Roadmap:** track G, T0

## Problem

Programs have only page blocks (`ALLOC`); text buffers and lists of the tools need a general allocator.

## Plan

- A `GlobalAlloc` in libmind over `ALLOC` blocks (size classes plus large blocks), opt-in per program; out-of-memory returns null, never panics in the allocator.

## Acceptance criteria

- Host tests of the allocator; a QEMU program allocates and frees until the heap quota and recovers.

## Related

[docs/tools/README.md](../docs/tools/README.md) (plan of the tools track, branch `claude/wizardly-franklin-kec1a9`) F1.
