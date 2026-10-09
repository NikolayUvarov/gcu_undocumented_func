# 700-EFF-0009 — System update from the server, only through `updater`

**Type:** service feature · **Owner:** `EFF` track (open) · **Priority:** P2 · **Status:** open · **Blocked by:** [700-EFF-0007](700-EFF-0007-gateway-service.md); [351-UPD-0007](351-UPD-0007-updater-service.md) (the `updater` service); [requests-UPD.md](requests-UPD.md) (a client of `updater` for the gateway) · **Main task:** [700](700-effector-agent.md) · **Constitution:** MC-8.5, MC-9.3, MC-9.4, MC-9.9, MC-7.5

## Problem

- **What the server sends.** Effector's `update_agent` asks an agent to update itself.
- **Why MIND Core differs.**
  - The agent is part of the signed system image.
  - The only verifiable way to change the image is `updater` (351): a signed release, slots A and B, a trial boot, last-known-good.
  - The agent must not download and swap its own binary.

## Plan

- **`update_agent` → `update-check` (0003).** The gateway asks `updater` to check the owner's channel, and to fetch and stage a newer release if the policy allows.
- **Apply.** Only if the policy allows the restart. The trial boot and last-known-good protect the machine (351).
- **What the server cannot choose.** The channel, the keys and the release come from the owner's configuration, never from the server's command (MC-7.5).
- **The heartbeat's `update` field** maps the updater's states: idle, checking, staged, applying, trial, succeeded, failed, rolled back. It names the versions of the running, staged and last-known-good releases.
- **The result string.** The one the contract gives for "no update service" while `updater` is absent or the policy forbids it.

## Acceptance criteria

- **In the `effector` suite, with the `updater` suite's release server:**
  - `update_agent` stages a newer test release;
  - with the policy's permission, the machine restarts into it, confirms the trial, and the heartbeat shows the new version;
  - without permission the release stays staged and the result says so;
  - a rollback to last-known-good is reported.

## Related

[351](351-self-update.md), [351-UPD-0007](351-UPD-0007-updater-service.md), [docs/update/slots.md](../docs/update/slots.md).
