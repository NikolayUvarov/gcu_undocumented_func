# 801-EFF-0038 — The heartbeat throughout a download

**Type:** session · **Owner:** `EFF` · **Priority:** P1 · **Status:** in progress (2026-11-21) · **Blocked by:** — · **Main task:** [801](801-the-bench-stays-manageable.md) (S5, S7) · **Constitution:** MC-12.1

## Problem

KRN's request of 2026-11-17, "The heartbeat throughout an update's download" in `issues/requests-EFF.md`, taken here.

- **On the DEXP** (801, "The DEXP's first release through Effector"), from the moment `update_agent` was taken (21:25:36 UTC), the server saw no heartbeat for about 8 minutes, until the agent's `shutting_down` at 21:33:55.
- **Why:** the session's loop sends the heartbeat between requests, and a download holds the loop and the connection until its body ends.

## Plan

1. **`send` takes the connection out of the agent for the request it carries.** Between two chunks of a download's body, a heartbeat that is due goes on another connection (`aside`), on the session's period (1.5 s).
   - The heartbeat says the agent is busy with the command, as for any answer that waits.
   - Afterwards one connection stays; the other is closed.
2. **The pauses between a cut download's tries** (801-EFF-0037) send heartbeats the same way, and init's stop notice ends them at once.
3. **The `effector-update` suite:** while the resumed download runs, the fixture receives at least one heartbeat for every 3 s of it.

## Acceptance criteria

- [ ] The `effector-update` suite, x86 and aarch64, as above.
- [ ] On the bench: an `update_agent` over Wi-Fi with the server's heartbeats seen throughout (MC-12.1).

## Related

[801-EFF-0037](801-EFF-0037-a-cut-download-goes-on-from-where-it-stopped.md), [750-EFF-0006](750-EFF-0006-session-service.md).
