# 300-STO-0011 — A client that may only store, refused a read on the platform

**Type:** service (storage) · **Owner:** `STO` track · **Priority:** P3 · **Status:** open · **Blocked by:** a client badged `BADGE_PUT` alone (to be requested from `KRN` when a program needs one) · **Main task:** [300](../issues-done/300-checksummed-block-store.done) · **Roadmap:** track B · **Constitution:** MC-4.7, MC-4.11

Split from [300-STO-0004](../issues-done/300-STO-0004-rights-by-badge.done), whose platform check covers a client that may only read.

## Problem

The rule that storing does not grant reading (MC-4.7, 4.11) is host-tested for a client badged `BADGE_PUT` alone. No such client exists on the platform: `init` mints the shell's full client and its get-only client, and no program asks only to store.

## Plan

- When a program needs to store without reading (a collector of logs, a backup writer), the kernel track is asked for a put-only client and a request flag, as 300-KRN-0024 did for reading.
- The `store` suite then checks that such a client's `get`, `resolve` and `publish` are refused and logged.

## Acceptance criteria

In the `store` suite, a client badged `BADGE_PUT` alone stores and is refused `get`, logged with its badge.

## Related

[300-KRN-0024](../issues-done/300-KRN-0024-read-only-blockstore-client.done), [docs/storage](../docs/storage/README.md).
