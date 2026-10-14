# 303-STO-0017 — The collection check waits for the lease by the guest's clock

**Type:** storage (tests) · **Owner:** `STO` track · **Priority:** P3 · **Status:** in progress (made; the gate's runs are left) · **Blocked by:** — · **Main task:** [303](../issues-done/303-retention-and-collection.done) · **Roadmap:** track B · **Constitution:** MC-5.6, MC-12.2

From the request "The block store's collection check waits 61 s by the host's clock for a 60 s lease" in requests-STO.md (recorded by the drivers track, 2026-10-12).

## Problem

The store suite's collection check put a block, slept 61 s by the host's clock, then expected the next put to collect it. The lease is 60 s by the guest's clock. In two of the drivers track's local gates, with many guests running at once, a guest fell behind the host by more than a second over that minute, the lease had not ended, and the check got `GOT 12 BYTES` instead of `NotFound`. It passed when the group ran alone.

## Plan

Read the guest's `uptime` right after the put, sleep 55 s, then poll `uptime` every 2 s until 62 s have passed by the guest's clock. That is 62 rather than 61 because `uptime` counts whole seconds. The wait follows the guest's clock instead of a longer fixed sleep (MC-5.6).

## Acceptance criteria

The check passes under a full local gate as it does alone.

## Related

[303-STO-0001](../issues-done/303-STO-0001-collection-by-reachability.done), [300-STO-0016](300-STO-0016-a-dag-walked-once.md) (the same suite's timing on a slower machine).
