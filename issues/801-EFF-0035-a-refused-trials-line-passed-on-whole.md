# 801-EFF-0035 — A refused trial's line reaches the server whole: `effector.wit` 2.0's `update-state` failure of 256 bytes

**Type:** interface · **Owner:** `EFF` · **Priority:** P1 · **Status:** in progress (2026-11-15) · **Blocked by:** — · **Main task:** [801](801-the-bench-stays-manageable.md) (S2) · **Constitution:** MC-12.4, MC-12.7

## Problem

UPD's request of 2026-11-15, for [801-UPD-0042](801-UPD-0042-a-refused-trials-reason-reaches-the-server-whole.md), which lands with it. It was recorded in `issues/requests-EFF.md` and is taken here.

- **What the `rollback` suite saw** (x86 and aarch64): slot A's heartbeat reported `version 6 in slot B, falling back to slot A: NO HEARTBEAT ACCEPT`.
- **What was lost:** the reason and the agent's last step, which S2 asks for, were cut off.
- **Why:** `update.wit` 1.5's `state` and `effector.wit` 1.6's `update-state` both bound the failure at 64 bytes.
- **UPD's part:** `update.wit` 2.0 makes the updater's failure `string<256>`.
- **Why the two land together:** with UPD's part alone, the gateway's `Text::new` of a line over 64 bytes gives an empty one. The agent would then report no fallback at all.

## Plan

1. **`effector.wit` 2.0:** `update-state`'s `failure` is `string<256>`.
   - A larger bound in a record is a new major version (MC-12.4). A 1.x decoder refuses a longer line rather than reading it short.
   - The agent, the gateway and the shell are built together.
2. **The gateway passes the updater's line on whole.** It cuts at a character's edge only past 256 bytes, never giving an empty line.
3. **The agent's heartbeat buffer is 2048 bytes.** That holds a 256-byte line, escaped, beside the longest agent and command IDs and the machine's name.

## Acceptance criteria

- [ ] Host tests: `tests/effector_wire_host.rs`. A 256-byte line, mostly escaped, fits a heartbeat with the longest fields.
- [ ] UPD's `rollback` suite, x86 and aarch64, with UPD's `update.wit` 2.0: slot A's heartbeat and its answer carry the reason kept in `system/updater/refused`, whole.

## Related

[801-UPD-0042](801-UPD-0042-a-refused-trials-reason-reaches-the-server-whole.md), [801-EFF-0029](801-EFF-0029-the-fallbacks-report-on-the-first-connection.md), [801-EFF-0034](801-EFF-0034-the-kept-update-waits-for-inits-word.md).
