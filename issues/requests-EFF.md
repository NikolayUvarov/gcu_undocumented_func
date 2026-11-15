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

## UPD's request (2026-11-15): `effector.wit`'s `update-state` failure up to 256 bytes, passed on whole

For [801-UPD-0042](801-UPD-0042-a-refused-trials-reason-reaches-the-server-whole.md) (801, S2), which lands with it.

- **What the `rollback` suite saw** (x86 and aarch64): slot A's heartbeat reported `version 6 in slot B, falling back to slot A: NO HEARTBEAT ACCEPT`. The reason and the agent's last step, which S2 asks for, were cut off.
- **Why:** both records bound the failure at 64 bytes: `update.wit` 1.5's `state` and `effector.wit` 1.6's `update-state`.
- **UPD's part** (`claude/801-UPD-0041-a-release-that-loses-the-server-rolled-back`, 4269f9ec9d):
  - `update.wit` 2.0 makes the updater's failure `string<256>`;
  - the line is at most 44 bytes plus `init`'s reason of up to 96;
  - the updater gives its first 256 bytes, cut at a character's edge.
- **The request** (EFF decides how):
  - the gateway's `update-state` failure as long, in a new major version of `effector.wit`;
  - the gateway passing the updater's line on whole.
- **Why the two must land together:** today the gateway's `Text::new` of a line over 64 bytes gives an empty one. With UPD's part alone the agent would see no fallback at all, which is worse than a cut one. I will put both in the same round.
- **The check:** the `rollback` suite compares slot A's heartbeat and its answer to the update command with the reason in `system/updater/refused`, whole.
