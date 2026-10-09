# 501 — The effector: tests on real hardware, driven by Effector

**Type:** main task · **Owner:** `ASR` track (open) · **Priority:** P2 · **Status:** open · **Blocked by:** [700](700-effector-agent.md) (the agent: [700-EFF-0006](700-EFF-0006-session-service.md), [700-EFF-0007](700-EFF-0007-gateway-service.md)); [550](550-network-on-real-hardware.md) (the network on the MacBook Pro); `KRN`: the test account's spawn rights (below) · **Roadmap:** Assurance "evidence tied to configuration" · **Constitution:** MC-12.1, MC-12.2, MC-12.4, MC-12.9, MC-2.3, MC-10.2, MC-11.9

Asked by the maintainer (2026-10-08): tests should run on real hardware without the maintainer typing them. Opened by the kernel session; the `ASR` track is open.

**Revised at the maintainer's request (2026-10-09):** the test server is Effector, the maintainer's existing management system, so no server or protocol of our own is written.

**Reconciled the same day with main task [700](700-effector-agent.md)** (track `EFF`), which the maintainer opened for the agent itself: MIND Core managed from Effector, with software, updates, diagnostics and services. The agent, its contract and its runs against Effector are 700's. This task keeps what tests need on top of it: scripts run in a test account, and each run recorded per configuration.

## Problem

Every check on a real machine today is typed by the maintainer and read from photographs of the screen ([211-PRT-0004](211-PRT-0004-first-run-on-an-intel-pc.md)). The QEMU suites (`tests/qemu_smoke.py`) drive a guest through its serial line and QMP, which a laptop does not have. Evidence for a physical configuration is therefore rare, slow and made by hand. MC-12.1 asks for evidence per configuration, and QEMU's results do not carry over to a machine (MC-12.9).

## Effector

Effector (`sst-test-deploy`) is the maintainer's project, outside this repository. It is a fleet server written in Go, with a web interface and an operator REST API (`/api/v1/operations`, described in its `GET /api/openapi.json`).

**Its agents** (Windows, Linux) connect out over HTTPS:
- a heartbeat every second;
- commands taken from a wake-up stream and by polling;
- an acknowledgement with each result;
- files and packages over routes of their own.

**How operator operations reach an agent:**

| Operator operation | Agent action |
|---|---|
| `test.run`, `command.exec` | `exec_v1` |
| `logs.collect` | `collect_logs` |
| `screen.capture` | `screenshot` |
| `software.install` | `deploy` |
| `software.update_agent` | `update_agent` |

The agent routes are not in the OpenAPI file. The subset MIND Core implements is fixed in [700-EFF-0001](700-EFF-0001-protocol-contract.md).

**Trust today.** Its agents pin the server by the SHA-256 of its leaf certificate, with a second pin for rotation. They present a token the server gives the whole fleet. Effector does not yet accept an agent by a key of its own device; that is Effector's task 75, outside this repository.

## Design (proposed)

**The agent is 700's:** `effector` speaks to the server, `effector_gw` acts on the machine within the owner's policy. It gives tests:
- pinned HTTPS;
- the device key ([351-NET-0005](../issues-done/351-NET-0005-persistent-device-key.done), kept across boots) as a client certificate once Effector accepts it;
- a heartbeat carrying the device key's fingerprint and the image's manifest hash (350);
- logs from `logd` under a read grant (MC-10.2), and screenshots as the shell's `screenshot` takes them (086), where the policy allows them;
- new images only through `updater` (351) with the trial boot and last-known-good. The agent never writes disks or boot files itself; which key may sign a test release is a key-role question for 351-UPD-0009.

**Test runs, this task's part.**
- An `exec_v1` that comes from `test.run` or `command.exec` carries an `msh` script.
- When the owner's policy enables test runs, `effector_gw` runs the script in a **test account**:
  - only the grants its `requires:` line declares and the policy allows;
  - within a memory and time budget;
  - its output, exit status and time come back as the operation's result.
- Never a shell with the agent's own capabilities. Without the policy line the action is answered `denied`.
- A test gets none of the agent's capabilities (TLS, the device key, the token, the update request) and cannot widen its grants (MC-11.9).

**Evidence.**
- Each run is recorded with its configuration: the machine, the firmware and its settings, the image's manifest hash (350), the script and any seed.
- Effector keeps the operation's result, and the profile cites a run for that configuration only.
- A passing run is evidence of what was run, not a proof (MC-12.2).

**Its own risk.** The agent lets Effector's operators run scripts on the device, within the test account. It is off by default and turned on by the machine's owner, and test runs need their own policy line. [docs/profile/threat-model.md](../docs/profile/threat-model.md) names it when it lands.

## What exists (2026-10-09)

| Need | State |
|---|---|
| An HTTP client | `libmind::http` ([351-NET-0001](../issues-done/351-NET-0001-http-downloads.done)): one GET with `Range`, its head parsed by the `parse` service ([109](../issues-done/109-session-parsers.done)). No POST, no keep-alive; chunked bodies and redirects are refused ([requests-NET.md](requests-NET.md)) |
| TLS for a program or service | `REQUEST_TLS`, the kernel's part ([351-KRN-0034](../issues-done/351-KRN-0034-a-tls-client-for-programs.done)); the shell lending its client is the tools track's (requests-APP). A service gets its TLS client from `init`, as `updater` will ([351-KRN-0022](351-KRN-0022-updater-grants.md), open) |
| HTTPS with a pinned server | [351-NET-0002](351-NET-0002-https-for-programs.md): on the network track's branch, not yet in `main`, `tls.wit` 1.1 `connect-pinned` checks one SHA-256 of the server's public key (SPKI). Effector's agents pin the leaf certificate, current and next: either Effector also publishes SPKI pins (its task 75) or `tls` takes the leaf kind and a second pin ([requests-NET.md](requests-NET.md)) |
| A client certificate | `tls` offers the device's Ed25519 certificate when the server asks (`tls/src/device.rs`) |
| The device key across boots | [351-NET-0005](../issues-done/351-NET-0005-persistent-device-key.done), done: on the disk in `keystore`'s private directory; a TPM next (351-NET-0006) |
| Host names in the network policy, changed while the system runs | [351-NET-0003](../issues-done/351-NET-0003-names-in-the-network-policy.done), [108](../issues-done/108-editable-network-policy.done), done |
| Parsing external input | the `parse` service and the authority map ([109](../issues-done/109-session-parsers.done), [docs/network/airlock.md](../docs/network/airlock.md)) |
| Scripts with declared grants | `msh` and its `requires:` line ([docs/msh.md](../docs/msh.md)) |
| A screenshot | [086](../issues-done/086-screenshot.done), done |
| Updates | slots, boot records, the trial and the fallback ([351-UPD-0006](../issues-done/351-UPD-0006-slots-and-boot-records.done), [351-KRN-0014](../issues-done/351-KRN-0014-trial-boot-and-confirmation.done)); the `updater` service ([351-UPD-0007](351-UPD-0007-updater-service.md)) open |
| The network on the MacBook Pro | [550](550-network-on-real-hardware.md), open; the USB Ethernet adapter first ([550-DRV-0005](550-DRV-0005-usb-ethernet.md)) |
| JSON | nothing in the tree; [700-EFF-0004](700-EFF-0004-json.md) |
| The agent itself | nothing yet; [700](700-effector-agent.md) |

## Plan: tasks

| Task | Track | What |
|---|---|---|
| 501-ASR-0007 | `ASR` (open) | **Moved to 700 (2026-10-09), not written.** The profile of the protocol is [700-EFF-0001](700-EFF-0001-protocol-contract.md), its test server [700-EFF-0002](700-EFF-0002-test-server.md), JSON [700-EFF-0004](700-EFF-0004-json.md). The run against Effector's own server is [700-EFF-0012](700-EFF-0012-run-against-the-effector-server.md); Effector is a private project, so that run is made on the maintainer's machine, not in CI |
| 501-ASR-0008 | `ASR` (open) | **Moved to 700, not written.** The agent service is [700-EFF-0006](700-EFF-0006-session-service.md) and [700-EFF-0007](700-EFF-0007-gateway-service.md) |
| 501-ASR-0009 | `ASR` (open) | Test runs: `msh` scripts in a test account in `effector_gw`, under the policy. Also a host driver that queues the QEMU suites' checks through Effector's operator API and records each run with its configuration. Written when 700-EFF-0007 lands |
| `init`'s grants for the test account (not numbered yet) | `KRN` | Spawn rights for the test account in `effector_gw`, numbered by the kernel track when 501-ASR-0009 needs them. The agent's own grants are 700's request in [requests-KRN.md](requests-KRN.md) |
| Effector's side | Effector | Its task 75: MIND Core as a platform of agents; agents accepted by device key, from a list; the actions MIND Core performs |

501-ASR-0007 and 501-ASR-0008 were numbered on 2026-10-08 and moved to 700 on 2026-10-09 without files; their numbers are not reused.

## Acceptance criteria

- **In QEMU, against the [700-EFF-0002](700-EFF-0002-test-server.md) test server, and once against Effector's own server (700-EFF-0012):**
  - a `test.run` with an `msh` script runs in the test account;
  - its output, the log and a screenshot come back as the operation's results;
  - a test that uses a capability its script did not declare is refused;
  - without the policy's test line the run is `denied`.
- **On the MacBook Pro, over the network of 550:** one of the QEMU suites' checks runs unattended from Effector, and its record names the machine's configuration.
- **Documentation:** [docs/assurance/README.md](../docs/assurance/README.md) and the profile say what the agent's runs cover, and that they are not proofs.

## Related

[700](700-effector-agent.md), [500](500-fuzzing-abi-and-idl.md), [350](350-signed-boot-images.md), [351](351-self-update.md), [550](550-network-on-real-hardware.md), [650](650-building-on-the-target.md) (builds through a server), [211-PRT-0004](211-PRT-0004-first-run-on-an-intel-pc.md), `tests/qemu_smoke.py`, [docs/profile/evidence.md](../docs/profile/evidence.md), [docs/msh.md](../docs/msh.md).
