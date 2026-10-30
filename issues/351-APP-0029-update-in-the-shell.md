# 351-APP-0029 — `update` in the shell and `msh`

**Type:** tools · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** in progress (back on the tools branch in dc67e0c5, 2026-10-30; the `update` suite's run left) · **Blocked by:** — (the updater takes buffer calls since `main` 57c2fd3d, 751-UPD-0032) · **Main task:** [351](351-self-update.md) (phase 2) · **Roadmap:** track G · **Constitution:** MC-9.2, MC-12.4

_Numbered 351-APP-0016 in the tools branch's commits until main's own 109-APP-0016 (2026-10-09); renumbered at the merge._

Numbered from the kernel track's request in `requests-APP.md` (2026-10-08, for 351 at the maintainer's request). That file went once every request in it was numbered.

## Problem

The `updater` service ([351-UPD-0007](../issues-done/351-UPD-0007-updater-service.done)) will have an interface, `idl/update.wit`, but no command to drive it. It does not exist yet, so nothing here can be built or tested until it lands.

## Plan

- **Shell commands:** `update check | fetch | apply | status | rollback`.
- **Scripts:** the same commands in `msh` under `requires: lifecycle`.
- **`apply`** asks for confirmation and names the running and the new version.
- **`status`** shows the running, staged, trial and last-known-good versions and the last error.
- **Monitors:** `sysmon` or `top` may show a staged update.
- The shell lends nothing new for this: it calls the updater with the client init gives it. Which client, and with what badge, is decided with 351-UPD-0007 and [351-KRN-0022](../issues-done/351-KRN-0022-updater-grants.done).

## Acceptance criteria

- In QEMU, the commands drive the updater against the test server of [351-UPD-0005](../issues-done/351-UPD-0005-release-and-publish.done).
- The tools suite checks:
  - each command's output;
  - a refused `apply` without confirmation;
  - a script without `requires: lifecycle`, which is refused.

## Related

[351](351-self-update.md), [351-UPD-0007](../issues-done/351-UPD-0007-updater-service.done), [351-KRN-0022](../issues-done/351-KRN-0022-updater-grants.done), [094](../issues-done/094-shell-script-language.done) (msh).

## Progress (2026-10-28)

- **Written** on `claude/wizardly-franklin-kec1a9`, in 6818cae8 and c56a11ef:
  - `update [status|check|fetch|apply [tries]|rollback]` through the shell's client in `SLOT_UPDATE` (351-KRN-0116);
  - `status` with `update.wit` 1.1 `metadata` (the update track's request for 351-UPD-0025, taken from requests-APP.md);
  - `apply` and `rollback` asked at the console. What the updater refuses whatever the other slot holds (no slot, a trial) is answered without a question; after a restart the question names the release in the other slot, which the updater checks on the disk;
  - errors in words; `requires: lifecycle` in scripts; no lent client;
  - the code in `shell/src/updates.rs` (text, host-tested) and `shell/src/updatectl.rs` (calls).
- **Tests then:**
  - host tests passed;
  - the `update` suite's checks were added: `status` before and after the trial, `check`'s refusal, a declined `rollback` and `apply`, a usage error, a script without `requires: lifecycle`.
- **Found by that suite** (x86, QEMU, TCG, 4 CPUs): the first `update` answered `UPDATE: THE UPDATER DOES NOT ANSWER`.
  - The updater receives with no capability slot (`recv_timeout(0, …)`), so every call that lends a buffer is refused: `check`, `status`, `metadata`.
  - The change is the update track's, requested in [requests-UPD.md](requests-UPD.md).
- **Taken out** in 39432555, so that 800-APP-0108 and 750-APP-0109 land without it. It comes back by reverting that commit once the updater's change is in `main`, with the suite's checks; then `fetch` and an agreed `apply` from the shell are still to be shown in QEMU. In the `update` suite the automatic policy does both, so that needs a run without `automatic apply`.

## Progress (2026-10-30)

- **The updater takes buffer calls.** It receives a caller's buffer in slot 9 since `main` 57c2fd3d, with KRN's 6a7ca80d for 751-UPD-0032 and `update.wit` 1.3. So `check`, `status` and `metadata` answer the shell's client now.
- **The command is back** in dc67e0c5, which reverts 39432555, with its host tests and its `update` suite checks. The help keeps the newer lines on pipes and redirection beside `update`'s.
- **Built** against `update.wit` 1.3; the host tests `tests/shell_host.rs` pass (21).
- **Left:**
  - the QEMU `update` suite;
  - KRN's suggestion for 751-UPD-0032: the shell's client calling `install-begin` is `denied`.
