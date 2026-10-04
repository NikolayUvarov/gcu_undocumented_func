# 039 — Port the services to MIND IDL and the C4 memory modes (roadmap C8)

**Type:** kernel · **Owner:** kernel track (coordinates `kernel/src/scheduler.rs`, `common/abi.rs`) · **Priority:** P1 · **Status:** open · **Roadmap:** step 3, C8 · **Constitution:** MC-2.3, MC-2.6

## Problem

Only `rtc` uses MIND IDL; the other services use numeric conventions and share heap blocks read-write.

## Plan

- Interfaces in `idl/`: `block.wit`, `audio.wit`, `tts.wit`, `loader.wit`, `init.wit` (needs IDL v0.2 from issue 044 for strings and buffers).
- Transfers: the client's buffer as a lease or sealed read-only object where the server only reads; read-write sharing only where declared in the profile.
- VFS is ported once as VFS v2 together with the tools track (issue 048).

## Acceptance criteria

- Every service decodes requests with generated bindings; no `MEM_SHARE` read-write buffer remains outside the profile's list.

## Related

[ROADMAP](../ROADMAP.md) C8; [044](044-idl-v02-records-strings.md), [048](048-write-path-ramdisk-vfs2.md).
