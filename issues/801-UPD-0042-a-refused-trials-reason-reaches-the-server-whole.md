# 801-UPD-0042 — A refused trial's reason reaches the server whole: the update state's failure up to 256 bytes

**Type:** update (`updater`, `update.wit` 2.0) · **Owner:** `UPD` (the kernel session) · **Priority:** P1 · **Status:** open (2026-11-15) · **Blocked by:** — (EFF's part, 801-EFF-0035, lands in the same round) · **Main task:** [801](801-the-bench-stays-manageable.md) (S2) · **Roadmap:** stage V · **Constitution:** MC-9.4, MC-12.4

## Problem

**The report loses the reason.** The `rollback` suite (801-UPD-0041) saw slot A's heartbeat report the fallback as `version 6 in slot B, falling back to slot A: NO HEARTBEAT ACCEPT`, on x86 and aarch64.
- **Where it is cut:** the updater gives its failure in `update.wit` 1.5's `state`, as `string<64>`, so the updater cuts it to 64 characters. The gateway's `update-state` (`effector.wit`) has the same bound.
- **What is lost:** the line is `version V in slot S, falling back to slot T: ` (44 bytes) followed by `init`'s reason (up to 96 bytes, `init.wit` 1.13 `boot.why`). The agent's last step and its detail are lost: no network, no TLS, refused, or said nothing.
- **Why it matters:** S2 requires that step in the report.

## Plan

- **`update.wit` 2.0:** the state's failure is `string<256>`.
  - **The version:** a field's bound changes the record's layout, so it is the major version (docs/idl). A reader of 1.x is answered as another major version, and every client is built with it.
  - **The updater** gives the failure's first 256 bytes, cut at a character's edge.
- **EFF's part** (requested in `issues/requests-EFF.md`): `effector.wit`'s `update-state` failure as long, and the gateway passing it on whole.
  - **Why it is needed:** today the gateway's `Text::new` of a longer line gives an empty one, which would lose the report altogether.
  - **When it lands:** EFF's part lands with this one, in the same round.

## Acceptance criteria

- **The `rollback` suite** (801-UPD-0041) checks slot A's heartbeat against the reason in `system/updater/refused`, whole; it passes on x86 and aarch64 with EFF's part.
- **The updater groups and `host tests`** pass, with the IDL check (`tests/idl_test.py`).

## Related

[801](801-the-bench-stays-manageable.md), [801-UPD-0041](801-UPD-0041-a-release-that-loses-the-server-rolled-back.md), [801-UPD-0037](../issues-done/801-UPD-0037-a-refused-trial-spent-and-its-reason-kept.done), [801-EFF-0029](801-EFF-0029-the-fallbacks-report-on-the-first-connection.md).

## Progress (2026-11-15)

- **UPD's part:** 4269f9ec9d, on 801-UPD-0041's branch.
  - **What changed:** `update.wit` 2.0, the updater's head of 256 bytes, the generated `libmind/src/idl/update.rs`, `docs/update/updater.md` and _RU 0.17, and docs/idl.
  - **What passed on the maintainer's machines:**
    - `host tests` on x32-128, with `tests/idl_test.py`;
    - `x86: the updater and an update through the link` on x32-128;
    - `aarch64: the updater and the modes` on acc-1.
- **With UPD's part alone** the `rollback` suite saw no fallback report at all on both arches: the gateway's `effector.wit` 1.6 bound of 64 bytes turns the longer line into an empty one. So this lands only with EFF's part, in the same round. It is not in `fast-test`, for the same reason.

## Progress (2026-11-15, with EFF's part)

**The tree:** `72e59d5685`. It is this branch with `main` (ABI 34) and EFF's `a736c064` merged in, which brings 801-EFF-0035: `effector.wit` 2.0, the gateway passing the updater's line on whole.

**The `rollback` suite passed on both arches** (x86 on x32 in 648 s, aarch64 on acc1 in 673 s). Slot A's heartbeat and its answer to the update command carry the reason in `system/updater/refused` whole.

**Also passed:**
- **the updater groups:** x86 on x32 in 1367 s, aarch64 on acc2 in 1298 s;
- **`host tests`,** with the IDL check: on x32, in 2217 s.

The rest of the runs are in [801-UPD-0041](801-UPD-0041-a-release-that-loses-the-server-rolled-back.md).
