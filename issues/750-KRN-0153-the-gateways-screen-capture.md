# 750-KRN-0153 — The Effector gateway's client of the screen

**Type:** `init` · **Owner:** kernel session · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [750](750-effector-agent.md) · **Constitution:** MC-3.4, MC-10.2

From EFF's request of 2026-10-22 (`KRN/2026-10-22T1650Z-EFF-0119-on-the-macbook-…` on `internal-dev-messages`).

## Problem

- **What the maintainer asks.** The maintainer asks the Effector server for a screenshot of the machine.
- **Why it cannot answer.** `effector_gw` holds nothing to capture with, so it answers `open-screen` `unsupported`.
- **What the policy already says.** The owner's policy names the screen (`screen on` or `off`), and the gateway audits each request. Only the grant is missing.

## Plan

1. **The grant.** In `effector_gw`'s arm, `init` lends the compositor's client in `SLOT_DISPLAY`, as the shell has it for `screenshot` and lends it for `REQUEST_DISPLAY`.
   - The client offers `display.wit`'s `mode`, `capture` (a sealed read-only copy of the screen) and the camera mark.
   - The compositor shows its red dot while it captures, so the person at the machine sees each capture.
2. **Only in a mode,** as the gateway's other grants are.
3. **The owner's word stays the gateway's.** `screen on` in the policy decides each request, and the audit records it (750-EFF-0007).

## Acceptance criteria

- [x] The `modes` suite (x86 and aarch64): in the development mode `effector_gw` holds the compositor's client in slot 15, unbadged; without a mode it does not run.
- [x] `docs/profile/bootstrap.md` names the grant.

## Progress (2026-10-22)

Made on `claude/youthful-mendel-mf1soy` (784e6a8f); the `modes` suite passed on x86 and aarch64 in this session's container (QEMU TCG). It closes once it is in `main`, with EFF's joint landing request.

## Related

[750-KRN-0119](750-KRN-0119-boot-images-and-grants-for-effector.md), [750-EFF-0007](750-EFF-0007-gateway-service.md), `idl/display.wit`, issue 086 (the shell's `screenshot`).
