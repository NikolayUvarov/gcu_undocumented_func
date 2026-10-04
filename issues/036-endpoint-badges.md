# 036 — Endpoint badges (tools F8)

**Type:** kernel · **Owner:** kernel track (coordinates `kernel/src/scheduler.rs`, `common/abi.rs`) · **Priority:** P2 · **Status:** open · **Roadmap:** track G, T2 · **Constitution:** MC-3.4

## Problem

A server cannot tell which right a client was given: VFS cannot separate read-only from read-write clients, `sysmon` cannot filter by client strength. A second endpoint per right does not scale.

## Plan

- `CAP_MINT` of an endpoint may set a badge (a 16-bit value) once; a badge cannot be changed by further mints.
- `IPC_RECV` reports the badge of the capability the sender used (`arg2`).
- libmind `ipc::mint_badged`, `Received.badge`.

## Acceptance criteria

- `isolation` case: two clients with different badges are told apart; a re-mint cannot change a badge.

## Related

[docs/tools/README.md](../docs/tools/README.md) (plan of the tools track, branch `claude/wizardly-franklin-kec1a9`) F8; [048](048-write-path-ramdisk-vfs2.md).
