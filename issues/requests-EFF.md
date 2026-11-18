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

## Requests from MIND Core's tracks, not numbered yet

The EFF track numbers its own tasks, so a request from another track waits here until EFF turns it into a task and removes it.

### The agent sets the clock from the pinned server's time (KRN, 2026-11-17, for 801's S5)

- **Why.** The maintainer chose (2026-11-17) that the agent sets the device's clock from the server's date, rather than a `timed` line in the default policy. 801-EFF-0027 judges commands by the server's time, but the device's clock stays wrong for the updater's expiry checks, other servers' certificates, NTS and the logs.
- **What KRN gives:** [801-KRN-0255](801-KRN-0255-the-agent-may-set-the-clock.md), `rtc` with `mind::rtc::BADGE_SET` in the agent's slot 2 (`rtc.wit` 1.3 `set`). Reading the clock through it is as before.
- **What is asked:**
  1. From the `Date` of an answer of the pinned server (the one `clock_from` reads), set the clock with `rtc`'s `set` when the device's clock is more than five minutes off it. Within five minutes, leave it to `timed` or the user.
  2. Never before the build's commit time (`MIND_SOURCE_TIME`, as `timed`'s floor), never past 2099; a `Date` outside them is not used.
  3. Log each set once, with the time before and after: for example `EFFECTOR: THE CLOCK SET FROM THE SERVER'S TIME: <before> -> <after>`.
  4. After a set, the offset of 801-EFF-0027 starts again from the next answer, so the commands' terms stay right across it.
  5. Tests: a host test of the rule (the threshold, the floor, 2099); the `effector` suite with the guest's clock 3 h ahead and at 2000-01-01, the clock read back within a few seconds of the fixture's after the first answer, and the log line.
- **What it trusts.** The server, or whoever holds its pinned key, can move the clock forward without limit and back to the floor ([clocks.md](../docs/profile/clocks.md), "The Effector server's time"): a memory domain's term in mode `forget` can then pass. If EFF wants the owner to be able to switch it off, a policy line (for example `clock keep`) would do; KRN does not ask for one.
- **Docs.** `docs/effector/README.md`'s table of the session's grants can then say `rtc` with the setting badge, and what the agent does with it.
### The heartbeat throughout an update's download (KRN, 2026-11-17, for 801's S5 and S7)

- **What the DEXP showed** (801, "The DEXP's first release through Effector"): from the moment `update_agent` was taken (21:25:36 UTC) the server saw no heartbeat for about 8 minutes, until the agent's `shutting_down` at 21:33:55. The bundle was 29 MB over the DEXP's Wi-Fi.
- **What is asked:** the heartbeat goes on, on its own schedule, while a bundle downloads and streams to the updater, as 750's "independently scheduled heartbeat" says; the update state in it (`receiving`) if the contract has the field. A test in the `effector-update` suite: with the fixture's download slowed to take longer than two heartbeat periods, the fixture still receives a heartbeat in each period.
