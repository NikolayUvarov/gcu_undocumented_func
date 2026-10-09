# 351-APP-0016 — `update` in the shell and `msh`

**Type:** tools · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open · **Blocked by:** [351-UPD-0007](351-UPD-0007-updater-service.md) (the `updater` service and `idl/update.wit`) · **Main task:** [351](351-self-update.md) (phase 2) · **Roadmap:** track G · **Constitution:** MC-9.2, MC-12.4

Numbered from the kernel track's request in `requests-APP.md` (2026-10-08, for 351 at the maintainer's request). That file went once every request in it was numbered.

## Problem

The `updater` service ([351-UPD-0007](351-UPD-0007-updater-service.md)) will have an interface, `idl/update.wit`, but no command to drive it. It does not exist yet, so nothing here can be built or tested until it lands.

## Plan

- **Shell commands:** `update check | fetch | apply | status | rollback`.
- **Scripts:** the same commands in `msh` under `requires: lifecycle`.
- **`apply`** asks for confirmation and names the running and the new version.
- **`status`** shows the running, staged, trial and last-known-good versions and the last error.
- **Monitors:** `sysmon` or `top` may show a staged update.
- The shell lends nothing new for this: it calls the updater with the client init gives it. Which client, and with what badge, is decided with 351-UPD-0007 and [351-KRN-0022](351-KRN-0022-updater-grants.md).

## Acceptance criteria

- In QEMU, the commands drive the updater against the test server of [351-UPD-0005](../issues-done/351-UPD-0005-release-and-publish.done).
- The tools suite checks:
  - each command's output;
  - a refused `apply` without confirmation;
  - a script without `requires: lifecycle`, which is refused.

## Related

[351](351-self-update.md), [351-UPD-0007](351-UPD-0007-updater-service.md), [351-KRN-0022](351-KRN-0022-updater-grants.md), [094](../issues-done/094-shell-script-language.done) (msh).
