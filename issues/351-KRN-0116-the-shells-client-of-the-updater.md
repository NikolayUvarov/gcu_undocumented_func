# 351-KRN-0116 — The shell holds a client of `updater` in `SLOT_UPDATE`

**Type:** `init`, ABI (`common/abi.rs`, ABI 12, with 351-KRN-0115) · **Owner:** kernel session · **Priority:** P2 · **Status:** in progress (in `main` at 1b1436a; the `status` call waits for the tools track's `update`) · **Blocked by:** — · **Main task:** [351](351-self-update.md) · **Constitution:** MC-3.4, MC-9.5

## Problem

`updater` serves `idl/update.wit` 1.0 (`check`, `fetch`, `apply`, `rollback`, `status`), but no task holds a client of it: it acts only through the policy in `update.txt`. The tools track's `update` command ([351-APP-0029](../issues-done/351-APP-0029-update-in-the-shell.done)) needs one. The update track's request of 2026-10-10 is in [requests-KRN.md](requests-KRN.md).

## Plan

1. **A fixed slot `SLOT_UPDATE`** for the shell, in ABI 12 with `SLOT_SSH` (351-KRN-0115).
2. **`init` gives the shell an unbadged client.** The updater checks no badge: every request ends in a check of signed releases, and `apply` and `rollback` ask the user through the shell's command.
3. **Not a launch slot yet.** The shell does not lend it to programs until the tools track asks for `update` in `msh`; then a request flag is a later ABI step.
4. **When `updater` restarts,** `init` gives the shell a new client, as for the other services the shell holds.

## Acceptance criteria

- [ ] The `update` suite: `stat caps` shows the shell's client of `updater` in `SLOT_UPDATE`, and a `status` call through it answers.
- [x] Every other suite passes as before.

## Progress (2026-10-20)

Made, with ABI 12:
- **The slot.** `SLOT_UPDATE` (35) holds the shell's unbadged client of `updater`, given by `init` while the updater runs.
- **No program gets it.** It is not a launch slot; `tests/runtime.rs` checks that.
- **Restarts.** A restart keeps the client working, as for the shell's other clients: `init` keeps the endpoint.

The `tls` suite requires the shell's slot 35 to name the updater's endpoint (passes locally on x86).

Left:
- the gate;
- a `status` call through the client, which needs the tools track's `update` command (351-APP-0029).

**The gate (2026-10-20).** Gate g0131 passed (`c23e754` on Effector's machines; the two groups with the updater suite again on `1b1436a`), and `main` is at 1b1436a. The `tls` suite's check of slot 35 passed on x86 and aarch64.

## Related

[351-APP-0029](../issues-done/351-APP-0029-update-in-the-shell.done), [351-UPD-0007](../issues-done/351-UPD-0007-updater-service.done), [351-KRN-0115](../issues-done/351-KRN-0115-the-ssh-service-in-the-boot-services.done); for Effector's gateway, the client of `updater` in [750-UPD-0020](750-UPD-0020-the-gateways-client-of-the-updater.md) and [750-KRN-0119](../issues-done/750-KRN-0119-boot-images-and-grants-for-effector.done).
