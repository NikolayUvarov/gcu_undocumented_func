# 501 — The effector: tests on real hardware, driven by Effector

**Type:** main task · **Owner:** `ASR` track (open) · **Priority:** P2 · **Status:** open · **Blocked by:** [550](550-network-on-real-hardware.md) (the network on the MacBook Pro); `NET`: [351-NET-0002](351-NET-0002-https-for-programs.md) (HTTPS with a pinned server), [351-NET-0005](351-NET-0005-persistent-device-key.md) (a persistent device key) and the request "HTTP for a service, and a certificate pin" ([requests-NET.md](requests-NET.md)); `KRN`: `init`'s grants (below); Effector: the device-key and MIND Core task (below) · **Roadmap:** Assurance "evidence tied to configuration" · **Constitution:** MC-12.1, MC-12.2, MC-12.4, MC-12.9, MC-2.3, MC-10.2, MC-11.9

Asked by the maintainer (2026-10-08): tests should run on real hardware without the maintainer typing them. Opened by the kernel session; the `ASR` track is open. Revised at the maintainer's request (2026-10-09): the test server is Effector, the maintainer's existing test-management system, so no server or protocol of our own is written.

## Problem

Every check on a real machine today is typed by the maintainer and read from photographs of the screen ([211-PRT-0004](211-PRT-0004-first-run-on-an-intel-pc.md)). The QEMU suites (`tests/qemu_smoke.py`) drive a guest through its serial line and QMP, which a laptop does not have. Evidence for a physical configuration is therefore rare, slow and made by hand. MC-12.1 asks for evidence per configuration, and QEMU's results do not carry over to a machine (MC-12.9).

## Effector

Effector (`sst-test-deploy`, not part of this repository) is a server with a REST API and a web interface, written in Go. Its agents run on Windows and Linux machines and poll it over HTTPS:

- `GET /api/config`, then `POST /api/heartbeat` every second;
- `GET /api/commands/poll` and `POST /api/commands/ack`;
- typed operations through `/api/v1/operations`: `test.run`, `command.exec`, `files.deliver`, `files.upload`, `files.download`, `logs.collect`, `screen.capture`, `software.update_agent` and others. The server describes them in `GET /api/openapi.json`.

Its agents pin the server's certificate by the SHA-256 of the leaf certificate, with a second pin for rotation. Effector does not yet accept an agent by a key of its own device.

## Design (proposed)

- **The effector is Effector's agent for MIND Core:** a service on the target (`effector/`, proposed). Like Effector's other agents it connects out and polls; the target needs no listening port.
- **The profile** (501-ASR-0007) states which routes, fields and operations the agent implements, against which version of Effector, as a documented wire format in `docs/assurance/effector.md` (MC-2.3). A change on either side is a new version of the profile, with its transition (MC-12.4).
- **Trust in the server:** HTTPS only, the server's certificate pinned as Effector's agents pin it (the `NET` request). Effector's plain-HTTP port is not used.
- **Authentication with the device key** (351-NET-0005). `tls` already offers an Ed25519 client certificate when a server asks for one (`tls/src/device.rs`). Effector keeps a list of the devices it accepts, by public key (the Effector task). A credential shared by all agents may be sent as well where Effector requires it, but it does not identify the device and is not enough on its own: a run must name its machine (MC-12.1).
- **Operations on MIND Core:**

  | Effector | On MIND Core |
  |---|---|
  | `/api/config`, heartbeat | the device key's fingerprint, the image's manifest hash (350), CPUs, memory, services; `os_version` `mind-core`, `arch` `x86_64` or `aarch64` |
  | `test.run`, `command.exec` | an `msh` script or a program run in the test account: only the grants its `requires:` line declares, within a memory and time budget; its output, exit status and time come back. Never a shell with the agent's own capabilities |
  | `files.deliver`, `files.upload`, `files.download`, `diagnostics.collect_file` | files under the test directory in `data/`, through the agent's `vfs` client; other paths refused |
  | `logs.collect` | the system log from `logd`, under a read grant (MC-10.2) |
  | `screen.capture` | the screen, as the shell's `screenshot` takes it (086) |
  | `software.update_agent`, `software.install` | a signed release, staged and applied only by `updater` (351) with the trial boot and last-known-good. The agent never writes disks or boot files. Which key may sign a test release is a key-role question for 351-UPD-0009 |
  | services, the interactive console, terminal reservations, ATM detection, INI programs | not in the first profile; answered as unsupported |

- **A test gets none of the agent's own capabilities** (TLS, signing with the device key through `keystore`, the update request) and cannot widen its grants (MC-11.9).
- **Evidence.** Each run is recorded with its configuration: the machine, the firmware and its settings, the image's manifest hash (350), the script or bundle and any seed. Effector keeps the operation's result; the profile cites a run for that configuration only. A passing run is evidence of what was run, not a proof (MC-12.2).
- **Its own risk.** The agent lets Effector's operators run code on the device, within the test account. It is off by default, turned on by the machine's owner, and [docs/profile/threat-model.md](../docs/profile/threat-model.md) names it when it lands.

## What exists (2026-10-09)

| Need | State |
|---|---|
| An HTTP client | `libmind::http` ([351-NET-0001](../issues-done/351-NET-0001-http-downloads.done)): one GET with `Range`, its body streamed into a sink. No POST, no keep-alive; chunked bodies and redirects are refused |
| TLS for a program or service | `REQUEST_TLS`, the kernel's part ([351-KRN-0034](../issues-done/351-KRN-0034-a-tls-client-for-programs.done)); the shell lending its client is the tools track's (requests-APP). A service gets its TLS client from `init`, as `updater` will ([351-KRN-0022](351-KRN-0022-updater-grants.md), open) |
| HTTPS with a pinned server | [351-NET-0002](351-NET-0002-https-for-programs.md), open; it plans a pin by the server's public key (SPKI) |
| A client certificate | `tls` offers the device's Ed25519 certificate when the server asks (`tls/src/device.rs`) |
| The device key across boots | [351-NET-0005](351-NET-0005-persistent-device-key.md), open |
| Host names in the network policy | [351-NET-0003](../issues-done/351-NET-0003-names-in-the-network-policy.done), done |
| Scripts with declared grants | `msh` and its `requires:` line ([docs/msh.md](../docs/msh.md)) |
| A screenshot | [086](../issues-done/086-screenshot.done), done |
| Updates | slots, boot records, the trial and the fallback ([351-UPD-0006](../issues-done/351-UPD-0006-slots-and-boot-records.done), [351-KRN-0014](../issues-done/351-KRN-0014-trial-boot-and-confirmation.done)); the `updater` service ([351-UPD-0007](351-UPD-0007-updater-service.md)) open |
| The network on the MacBook Pro | [550](550-network-on-real-hardware.md), open; the USB Ethernet adapter first ([550-DRV-0005](550-DRV-0005-usb-ethernet.md)) |
| JSON | nothing in the tree |
| The effector itself | nothing yet |

## Plan: tasks

| Task | Track | What |
|---|---|---|
| 501-ASR-0007 | `ASR` (open) | The profile of Effector's agent protocol in `docs/assurance/effector.md`; JSON for the agent (own code, or a `no_std` crate recorded in THIRD_PARTY.md); a host test that runs Effector's server, built at a recorded commit, against a guest in QEMU |
| 501-ASR-0008 | `ASR` (open) | The agent service on the target: polling, the device key, the test account, the profile's operations, updates through `updater` |
| "HTTP for a service, and a certificate pin" ([requests-NET.md](requests-NET.md)) | `NET` | POST with a body, keep-alive and chunked responses in `libmind::http`; a pin by the leaf certificate's SHA-256, current and next |
| `init`'s grants (not numbered yet) | `KRN` | TLS, a `netpolicy` flow to the Effector server, the log read grant, screen capture and the test account's spawn rights for the agent; numbered by the kernel track when 501-ASR-0008 needs them |
| Effector's task (in Effector's repository) | Effector | MIND Core as a platform of agents; agents accepted by device key, from a list; the profile's operations and their result records |

501-ASR-0007 and 501-ASR-0008 are numbered here; their files are written when work on them starts. 501-ASR-0007 needs none of the blockers: the profile and the QEMU test can start now. 501-ASR-0008 needs the `NET` and `KRN` parts for a real server, and 550 for the MacBook Pro.

## Acceptance criteria

- In QEMU, against Effector's server: a `test.run` with an `msh` script runs in the test account, and its output, the log and a screenshot come back as Effector's operation results. A test that uses a capability its script did not declare is refused. A server with another certificate is refused. A broken connection is resumed or reported.
- On the MacBook Pro, over the network of 550: one of the QEMU suites' checks runs unattended from Effector, and its record names the machine's configuration.
- [docs/assurance/README.md](../docs/assurance/README.md) and the profile say what the agent's runs cover, and that they are not proofs.

## Related

[500](500-fuzzing-abi-and-idl.md), [350](350-signed-boot-images.md), [351](351-self-update.md), [550](550-network-on-real-hardware.md), [650](650-building-on-the-target.md) (builds through a server), [211-PRT-0004](211-PRT-0004-first-run-on-an-intel-pc.md), `tests/qemu_smoke.py`, [docs/profile/evidence.md](../docs/profile/evidence.md), [docs/msh.md](../docs/msh.md).
