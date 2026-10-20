# 351-KRN-0116 — The shell holds a client of `updater` in `SLOT_UPDATE`

**Type:** `init`, ABI (`common/abi.rs`, ABI 11, with 351-KRN-0115) · **Owner:** kernel session · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [351](351-self-update.md) · **Constitution:** MC-3.4, MC-9.5

## Problem

`updater` serves `idl/update.wit` 1.0 (`check`, `fetch`, `apply`, `rollback`, `status`), but no task holds a client of it: it acts only through the policy in `update.txt`. The tools track's `update` command ([351-APP-0029](351-APP-0029-update-in-the-shell.md)) needs one. The update track's request of 2026-10-10 is in [requests-KRN.md](requests-KRN.md).

## Plan

1. **A fixed slot `SLOT_UPDATE`** for the shell, in ABI 11 with `SLOT_SSH` (351-KRN-0115).
2. **`init` gives the shell an unbadged client.** The updater checks no badge: every request ends in a check of signed releases, and `apply` and `rollback` ask the user through the shell's command.
3. **Not a launch slot yet.** The shell does not lend it to programs until the tools track asks for `update` in `msh`; then a request flag is a later ABI step.
4. **When `updater` restarts,** `init` gives the shell a new client, as for the other services the shell holds.

## Acceptance criteria

- [ ] The `update` suite: `stat caps` shows the shell's client of `updater` in `SLOT_UPDATE`, and a `status` call through it answers.
- [ ] Every other suite passes as before.

## Related

[351-APP-0029](351-APP-0029-update-in-the-shell.md), [351-UPD-0007](../issues-done/351-UPD-0007-updater-service.done), [351-KRN-0115](351-KRN-0115-the-ssh-service-in-the-boot-services.md); for Effector's gateway, the client of `updater` in [requests-UPD.md](requests-UPD.md) and [750-KRN-0119](750-KRN-0119-boot-images-and-grants-for-effector.md).
