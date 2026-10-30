# 750-EFF-0022 — The server's health check, answered from the gateway's status

**Type:** service · **Owner:** `EFF` · **Priority:** P2 · **Status:** in progress (2026-10-29) · **Blocked by:** the Effector server: `health_command` `health` in its `programs-mindcore` (a task in sst-test-deploy's tracker) · **Main task:** [750](750-effector-agent.md) · **Constitution:** MC-10.2, MC-12.1

## Problem

The maintainer (2026-10-29): both MIND machines are to be reachable through Effector, and Effector's problems are to be fixed.

On hesere, `health_check` on either MIND machine is refused before it reaches the agent: `400 health check is not configured`.
- **How the server runs one.** It sends `exec_v1` with the program's `health_command`.
- **What is missing on each side:**
  - the server's program "MIND Core" has no `health_command`;
  - the agent denies every `exec` but `reboot`.

## Plan

1. **`effector`:**
   - `exec_v1` of `health` asks the gateway's `status`; it needs no policy line, since it reads nothing of the machine's data;
   - **healthy** when the owner's policy is loaded and no boot service is quarantined: `completed`, exit status 0;
   - **otherwise** `failed`, exit status 1, `error` `exit status 1`, as the server's other agents report a health command that exits 1;
   - **`stdout`** is one line: the verdict, the uptime, the boot services running, and what is quarantined or missing;
   - the result is kept until the server takes it, as 750-EFF-0021's is.
2. **Contract 1.3** ([protocol.md](../docs/effector/protocol.md)) and [docs/effector/README.md](../docs/effector/README.md) describe it.
3. **The server:** a task in sst-test-deploy's tracker for `health_command: "health"` in the program "MIND Core" of `programs-mindcore`. The server's own agent does it.

## Acceptance criteria

- [x] The wire host test: a command that exits 1 is `failed` with its status and `error` (2026-10-29).
- [x] The `effector` QEMU suite on x86: `exec_v1` of `health` is answered `completed`, exit status 0, `stdout` starting `healthy: up` (4 CPUs, 2026-10-29; the agent logged `healthy: up 131 s, 28 services running`). It passes on aarch64 too (QEMU `virt`, e13b48b7).
- [ ] On both MIND machines, through hesere, once the server has the health command: `health_check` answers. Evidence for each machine only (MC-12.1).
  - **The MacBook Pro, 2026-10-29** (release 202610290937): the server's `exec` of `health` (operation `20261029095226.025109611_000348`) answered `completed`, exit 0, `healthy: up 88 s, 35 services running`.
  - **The DEXP, 2026-10-30** (release 202610290937, put over the cable): the server's `exec` of `health` (operation `20261030064641.716393471_000920`) answered `completed`, exit 0, `healthy: up 117 s, 35 services running`. Its `deliver_file` answered with the receipt as well (750-EFF-0021, operation `20261030064642.987154110_000921`).
  - **Left:** the server's `health_command`, asked in its inbox (`20261029T084429.186116961Z-fa687b3f56ecd4dd`). On 2026-10-30 06:50 UTC it is still `incoming`.

## Related

[750-EFF-0021](../issues-done/750-EFF-0021-a-delivery-answers-with-the-file-read-back.done) (a result kept until the server takes it), [750-EFF-0014](750-EFF-0014-telemetry.md).
