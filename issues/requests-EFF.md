# Effector server handoff: native MIND agent and signed-package contract

**Owner:** MIND Core EFF track, coordinating NET/KRN/UPD/APP dependencies · **Priority:** P1 · **Status:** open (UPD numbered its two parts on 2026-10-20: the package contract, draft 0.1, and the check as [750-UPD-0019](750-UPD-0019-signed-application-packages.md), the updater's client for the gateway as [750-UPD-0020](750-UPD-0020-the-gateways-client-of-the-updater.md)) · **Recorded:** 2026-10-15, at the maintainer's explicit request, relayed by the Effector server project · **Main task:** [750](750-effector-agent.md) · **Constitution:** MC-7.5, MC-8.5, MC-9.2, MC-12.1–12.4, MC-12.7, MC-12.9

## Ownership and existing work

The maintainer explicitly assigns native-agent implementation to MIND Core,
not to the Effector server project. Continue the existing tasks
[0003](../issues-done/750-EFF-0003-interface-and-policy.done),
[0006](750-EFF-0006-session-service.md),
[0007](750-EFF-0007-gateway-service.md),
[0008](750-EFF-0008-application-packages.md) and
[0012](750-EFF-0012-run-against-the-effector-server.md), with
[501](501-effector.md) for test-account scripts. This request creates no new
task numbers and claims no EFF track ownership for the external project.

The MIND `ASTRA` branch at `1aea9100` contains the initial contract and HTTPS
test fixture (`docs/effector/protocol.md`, `tests/effector_server.py`;
`bc48775d`, followed by `17e7005c`). They explicitly do not claim a working
native agent. Keep the delivered implementation and its evidence distinct
from these fixture and contract artifacts.

## Available server work

Effector (`sst-test-deploy`) canonical local master contains server change
`be9d62c33712c8576210fdf8362a7409f49ad4ae`, following task-82 investigation
`3dedf8db384198bf34414c37b0296f28055c3add`. These are local source commits;
no server publication, agent registration or live acceptance is claimed.

- `programs-mindcore` isolates discovery and operation lookup from host OS lists.
- Explicit `file_transfer.mindcore_directories` permits canonical `data/`
  and `log:/` paths; delivery is data-only, with no default permitted roots.
- MIND native binary metadata is unsupported; no Linux/Windows update fallback.
- Both package-queueing APIs reject MIND legacy script deployment with HTTP
  400 and `unsupported_platform`; a mixed batch queues no partial work.
- Server handler fixtures cover the file round trip, exact bytes/size/hash,
  ownership, final ACKs and transfer refusal after the final result. All eight
  MIND fixtures and the full server Go suite passed. They test the server only.

Effector's open tasks `81-mindcore-01-agent-and-live-acceptance.task.md` and
`81-mindcore-02-package-metadata.task.md` retain live and signed-package scope.
The old reference to this server project's task 75 is obsolete; use task 81.

## Requested from MIND Core

1. Deliver `effector` and `effector_gw` under the existing tasks, with pinned
   HTTPS, private native AgentToken, independently scheduled heartbeat,
   bounded parsing, owner policy/audit and durable replay-safe results before
   ACK. Keep the initial shell denial until test-account msh and grants exist.
2. Coordinate the existing UPD ([750-UPD-0019](750-UPD-0019-signed-application-packages.md), [750-UPD-0020](750-UPD-0020-the-gateways-client-of-the-updater.md)) and [KRN](requests-KRN.md)
   requests and return a reviewed versioned signed-application contract:
   canonical signed bytes, application key role/trust, payload identity and
   architecture, native install/update/uninstall entries, rollback policy and
   loader verification. Effector will implement its server schema only after
   this agreement; legacy scripts and a transport hash do not establish it.
3. Agree how the command binds the exact downloadable archive's identity,
   size and SHA-256, and how unsupported/missing/altered metadata is refused.
   Preserve existing host package behavior. Define a version transition before
   changing the initial subset's payload/parser bounds or action semantics.
4. Supply a build/revision, manifest hash, owner setup procedure without
   secrets, and the supported x86 QEMU configuration. Then coordinate each
   physical machine separately. Native AgentToken provisioning uses the
   owner's private channel; an operator API key cannot replace it.
5. Record ten-minute online behavior under work, allowed msh/log/file actions,
   byte/hash checks, reconnect, repeated command IDs, failed ACK retry and
   forbidden action/path refusal. Separately record signed install/update/
   remove and refusal of altered/unsigned/older packages. Name server/MIND
   builds and configuration; leave prerequisites and unrun criteria open.

## Answer requested

Please reply through `internal-dev-messages`, naming this request, the MIND
tasks/owner handling it, the agreed contract location/version and the first
build available for joint acceptance. Keep the binding outcome in the tasks
and this request on main. Neither project should implement the other's agent
or claim that the handoff itself completes interoperability.

## KRN's request (2026-11-15): `init`'s confirmation asserted in the `effector-update` suite

For [801-KRN-0242](801-KRN-0242-a-trial-confirmed-on-the-effector-servers-word.md), criterion 2 (801, S1).

- **What is shown now:** in round 139's gate, both `effector-update` suites passed (x86 on i9, aarch64 on acc-1). Their trial of release 6, with the mode on and `data/effector/effector.conf` on the disk, was confirmed.
- **What is not shown yet:** that `init` confirmed it on the agent's `accepted`. With a server configured that is `init`'s only way, but the suite does not check it.
- **The request:** once the trial is confirmed, the suite checks `init`'s log (`dmesg -s init`) for `[INIT] TRIAL BOOT CONFIRMED: THE EFFECTOR SERVER ACCEPTED THE AGENT'S HEARTBEAT`, on both arches.
- **Blocks:** 801-KRN-0242's close.

## UPD's request (2026-11-15): the trial's agent waits for `init`'s word on the kept update, and leaves slot A's state as it was

For [801-UPD-0041](801-UPD-0041-a-release-that-loses-the-server-rolled-back.md) (801, S1 to S3), which it blocks.

- **What the `rollback` suite found** (x86 on x32, aarch64 on acc-1, `claude/801-UPD-0041-a-release-that-loses-the-server-rolled-back` 19fb97b17f):
  - **The update and the trial:** slot A took release 6 by `update_agent` and restarted. Slot B came up on trial with the server out of reach, and `init` held it for its 600 s bound and refused it; that part passed.
  - **What B's agent did in the meantime:** 180 s into the trial (`CONFIRM_WAIT_MS`) it answered the kept command `failed`, `not-confirmed` ("version 6 runs in slot B on trial, not confirmed"), wrote it to `system/effector/result`, `executed` and `journal`, and removed `system/effector/update`.
  - **So:** after the fallback, slot A no longer has its kept answer. It holds B's stale answer instead of "fell back".
- **Why it matters:**
  - **S3:** a trial changes nothing the slot before reads until it is confirmed.
  - **The successful case:** a trial whose first accepted heartbeat comes after 180 s is confirmed by `init`, but its command is answered `failed`.
- **The request** (a proposal; EFF decides how):
  - while `init` says the trial is `deciding` (`init.wit` 1.13 `boot`), the kept update's verdict waits, with no limit of its own, and nothing under `system/effector/` changes for it;
  - once `init` confirms, the answer is `completed`;
  - if `init` refuses, the machine restarts and the slot before answers `fell-back` from its kept answer.
- **The check:** the `rollback` suite passes on both arches. It compares every file slot A reads before and after the trial, and it checks slot A's answer to the command.
