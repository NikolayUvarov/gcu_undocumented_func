# 700 — The Effector agent: MIND Core managed by the Effector fleet server

**Type:** main task · **Owner:** `EFF` track (open) · **Priority:** P1 · **Status:** open · **Blocked by:** for phase 2 and later: [requests-NET.md](requests-NET.md) (HTTP for a service, a certificate pin and long-lived sessions; JSON in the parser service; a flow for a long-running service), [requests-KRN.md](requests-KRN.md) (boot images and `init`'s grants with private directories, a writable scoped `vfs` client, a lifecycle client limited to named services), [requests-UPD.md](requests-UPD.md) (an `updater` client for the gateway, signed application packages), [requests-APP.md](requests-APP.md) (the shell's `effector` command); outside this repository, the Effector server's support for a `mindcore` platform (its task 75) · **Roadmap:** stage V (distribution: "remote capabilities through a gateway", Article 7), track D (session parsers with minimal authority), track C (updates only through `updater`); no roadmap item yet · **Constitution:** MC-7.1, MC-7.4–7.6, MC-6.6, MC-6.9, MC-3.1, MC-3.4, MC-3.7, MC-3.8, MC-3.11, MC-11.3–11.6, MC-11.9, MC-11.11, MC-10.2, MC-10.3, MC-10.7, MC-8.5, MC-9.2, MC-9.4, MC-9.8, MC-2.3, MC-12.1–12.4, MC-12.7, MC-12.9

Asked by the maintainer (2026-10-09): a MIND Core machine should be managed by the maintainer's existing fleet server, Effector (project `sst-test-deploy`), as its Windows and Linux terminals are. The server installs software, updates applications, collects diagnostics and data, and controls services and restarts. Opened by the session that surveyed both projects; the `EFF` track is open.

**Revised the same day,** after the maintainer's review and 501's revision, which had planned the same agent inside 501: 700 holds the agent and its contract, and 501 builds its test runs on it. The revision brought in the device key as the machine's identity, the image's manifest hash in the heartbeat, test scripts in a test account, and screenshots. `main`'s new authority map (109) moved JSON parsing into the `parse` service.

## Problem

Effector manages its endpoints through an agent on each machine. The agent:

- connects out to the server over HTTPS with a pinned server certificate and a bearer token;
- sends a heartbeat every second or two;
- takes commands from a queue (a wake-up stream plus polling);
- acknowledges each command with a result;
- downloads deployment packages and uploads files and logs.

Its commands are typed actions:

- `start_service`, `stop_service`, `restart_service`;
- `collect_logs`, `collect_file`, `deliver_file`;
- `deploy` (install, uninstall, reinstall a package);
- `update_agent`, `cancel_command`;
- configuration refreshes;
- shell commands (`exec_v1`, `powershell_v1`, a console).

MIND Core has no such agent, and several of the parts it needs are missing (surveyed on 2026-10-09):

- **HTTP.** `mind::http` does GET only. It has no request headers of the caller's (so no `Authorization`), no request bodies, no chunked responses and no kept-alive connections (351-NET-0001).
- **Trust.** `tls` is TLS 1.3 with CA roots only. No program can pin a server certificate (351-NET-0002 plans an SPKI pin), and no program but `updater` can hold a TLS client yet.
- **Formats.** There is no JSON and no ZIP reader in userland. Under the authority map ([docs/network/airlock.md](../docs/network/airlock.md), 109), external input is parsed in the `parse` service, which today parses HTTP heads only.
- **Network grants.** A boot service's flow is granted once by `init` and ends with its term and volume: 3600 s and 16 MiB by default; a policy line can name more, and the owner changes the policy with `netpolicy add` (108). A service that heartbeats for days needs that line, or renewal.
- **Local authorities.**
  - Only the shell can read the log or write to `data/`. A service can have a private directory in `system/` (as `keystore` and `netpolicy` do since 351-NET-0005 and 108), which nobody else can open.
  - Only `updater` may ask `init` to reboot.
  - `init`'s lifecycle client is all-or-nothing.
- **Applications.** They are not verified by the loader (profile, "Not met"), and there is no package format for adding one at run time.

The agent must not turn the server into an unchecked remote administrator. Every remote authority needs provenance, scope, replay protection, a term, an audience and revocation (MC-7.5). The parser of untrusted network input may not also hold broad grants (MC-11.11). Data that leaves the machine leaves by an authorized declassification (MC-10.7). Code and system images change only through verifiable update (MC-8.5).

## Design

### Two services, on the network path of Appendix B.6 (MC-11.3, MC-11.11)

**`effector` — the session gateway.**
- It holds:
  - one `netpolicy` flow to the configured server;
  - a `tls` client that checks the pinned leaf certificate and offers the device's certificate;
  - a `parse` client;
  - `rtc`;
  - a read-only view of the owner's configuration;
  - its own private directory in `system/`, which keeps the agent token.
- It speaks HTTP/1.1 with the server. Like `download`, it parses nothing but the framing: the HTTP heads and the JSON bodies go to `parse` (MC-11.11, [docs/network/airlock.md](../docs/network/airlock.md)), and it checks the typed result against what it expects (MC-11.5).
- Each command becomes a typed request of `idl/effector.wit`. A command that has no typed form is answered `denied` without reaching anything else.
- It holds no local authority beyond its endpoint to the gateway.

**Trust both ways.**
- The server is trusted by its pinned leaf certificate, current and next.
- The machine presents the token Effector requires today. It also presents the device key ([351-NET-0005](../issues-done/351-NET-0005-persistent-device-key.done), kept across boots) as its client certificate, which identifies the machine once Effector accepts agents by device key (Effector's task 75).
- The heartbeat carries the device key's fingerprint and the image's manifest hash (350), so a run names its machine and image (MC-12.1).

**`effector_gw` — the gateway.**
- It holds the local authorities, each granted by `init` and narrowed by the owner's policy:
  - a lifecycle client for named services;
  - the log read grant;
  - `sysinfo`;
  - a `vfs` client scoped to its own directories;
  - an `updater` client;
  - the reboot badge;
  - screen capture, where the policy allows it;
  - its own private directory for its journal and audit trail.
- It checks every typed request against the policy, binding it to the permitted scope (MC-3.8). It then executes the request, records it in the audit trail (MC-10.3) and returns a typed result.
- It never sees HTTP or JSON from the network.

### Off by default, set up by the owner

- Neither service starts until the owner enables it (`data/services.txt`, 173).
- The owner writes the configuration and the policy with the shell's `effector` command (requests-APP): server URL, certificate pins, the token, and what the server may do.
  - `effector` takes the token into its private directory and removes the shell's copy.
  - The flow's policy line is added with `netpolicy add`, which asks the user (108).
- The server cannot change the policy or the server URL. Over an already verified connection it may rotate the certificate pin within the current-plus-next set, as Effector's protocol does (MC-7.5).

### What each Effector action does on MIND Core

| Effector action | On MIND Core | Default policy |
|---|---|---|
| heartbeat | identity, `os_version: "mindcore"`, `arch` `x64` or `arm64` (Effector's values), release version, the device key's fingerprint, the image's manifest hash, the boot services and their states (`init.list`), the update state, `system_state`, time | always |
| `start/stop/restart_service` | `init` for a service named in the policy; never `init` or the shell | services listed by the owner |
| `collect_logs` | the log ring or a `log:/bootNNNN.log`, by a source name in the policy | allowed sources only |
| `collect_file` | a file under a policy root | owner's roots only; the agent's own files never |
| `deliver_file` | written atomically under the delivery root, size and SHA-256 checked; never executable by that alone | delivery root only |
| `deploy` install / uninstall / reinstall | a signed MIND Core application package inside the Effector ZIP, verified, installed under `data/apps/`, switched atomically; the previous version kept | off until package keys are set |
| `update_agent` | asks `updater` to check, fetch and stage the owner's channel; applies only if the policy allows the restart | check only |
| reboot (when the server has it: Effector 74.12) | `init` reboot through the gateway's badge | off |
| `refresh_config`, `update_agent_config` | re-read; pin rotation only | always |
| `cancel_command` | cancels a queued or interruptible typed request | always |
| `exec_v1` from `test.run` or `command.exec` | an `msh` script in a test account, with only the grants it declares and the policy allows ([501](501-effector.md), 501-ASR-0009) | off |
| `screenshot` | the screen, as the shell's `screenshot` takes it (086) | off |
| `exec`, `exec_v1` from `health_check`, `powershell_v1`, `console_v1`, `console_users_v1`, `collect_registration`, `set_hostport` | `denied`: there is no shell with the agent's capabilities; remote code arrives only as a signed package or a test script | never |

### Behaviour

- **Delivery (MC-6.6, MC-7.1).**
  - Each command ID and its result are written to a journal before the ACK.
  - ACKs are retried until accepted or past the command's term, and a repeated ID gets the stored result.
  - A timeout is reported as unknown outcome, never as "not done".
- **Losing the server (MC-7.6).** Nothing local depends on it. The agent reconnects with back-off, and running work finishes or stops as its own policy says.
- **Data leaving the machine (MC-10.7).** Only what the policy names leaves. The token, keys and the agent's journal are never collectable.

### Compatibility

- **The contract.** The agent speaks the subset of the protocol fixed in 700-EFF-0001, pinned to the Effector build it was checked against (server 1.2.0.127, commit `5d42b29`, as a starting point).
- **Version changes (MC-12.4, MC-12.7).**
  - Unknown fields are ignored.
  - A server whose contract version the agent does not know is reported and not guessed at.
  - A change of the subset is a new contract version.
- **Server side.** The Effector server needs its own changes for a third operating system (its task 75): a `mindcore` program list and path rules, action capabilities, no shell actions, and a published contract. Until then a MIND Core endpoint appears online and accepts packages, but program-bound actions and file transfers do not work.

## Plan: tasks by phase

**Phase 1 — no other track needed (start now):**

| Task | Track | What |
|---|---|---|
| [700-EFF-0001](700-EFF-0001-protocol-contract.md) | `EFF` | The protocol subset the agent implements, its contract version and the action mapping, in `docs/effector/protocol.md` |
| [700-EFF-0002](700-EFF-0002-test-server.md) | `EFF` | A test server on the host (`tests/effector_server.py`) that serves that contract over HTTPS, with fault hooks |
| [700-EFF-0003](700-EFF-0003-interface-and-policy.md) | `EFF` | `idl/effector.wit` 1.0 between the two services, the policy file, the audit record, the threat analysis |
| [700-EFF-0004](700-EFF-0004-json.md) | `EFF` | `mind::json`: a bounded JSON reader and writer for `no_std` with `alloc` |
| [700-EFF-0005](700-EFF-0005-zip-reader.md) | `EFF` | `mind::zip`: a bounded ZIP reader (stored and deflate) |

**Phase 2 — the agent online (needs NET and KRN):**

| Task | Track | What |
|---|---|---|
| [700-EFF-0006](700-EFF-0006-session-service.md) | `EFF` | `effector`: configuration, identity, heartbeat, stream and poll, ACK journal, pin rotation, shutdown heartbeat |
| [700-EFF-0007](700-EFF-0007-gateway-service.md) | `EFF` | `effector_gw`: policy, audit, services, logs, files |

**Phase 3 — software and updates (needs UPD, KRN and the Effector server):**

| Task | Track | What |
|---|---|---|
| [700-EFF-0008](700-EFF-0008-application-packages.md) | `EFF` | Signed application packages: install, uninstall, update an installed application, keep the previous version |
| [700-EFF-0009](700-EFF-0009-system-update.md) | `EFF` | `update_agent` through `updater`; the update state in the heartbeat |
| [700-EFF-0010](700-EFF-0010-reboot.md) | `EFF` | Reboot through the gateway, when the server has the action |

**Phase 4 — the owner's guide and the evidence:**

| Task | Track | What |
|---|---|---|
| [700-EFF-0011](700-EFF-0011-guide-and-profile.md) | `EFF` | The owner's guide (EN, RU), the profile, the bootstrap and threat-model tables |
| [700-EFF-0012](700-EFF-0012-run-against-the-effector-server.md) | `EFF` | A run against the real Effector server: QEMU first, then real hardware after 550; recorded per configuration |

**Requests to the tracks with owners**, recorded in their files:

| Request | Track | Needed by |
|---|---|---|
| HTTP for a service, a certificate pin and long-lived sessions (shared with 501) | `NET` | 0006 |
| JSON in the parser service | `NET` | 0006 |
| A flow for a long-running service | `NET` | 0006 |
| Boot images and `init`'s grants for `effector` and `effector_gw`, with their private directories | `KRN` | 0006, 0007 |
| A writable `vfs` client scoped to a service's directories | `KRN` | 0007, 0008 |
| A lifecycle client limited to named services | `KRN` | 0007 |
| An `updater` client for the gateway | `UPD` | 0009 |
| Signed application packages: format, key role, verification | `UPD` | 0008 |
| The shell's `effector` command | `APP` | 0006, 0011 |

### Order

```
0001 → 0002 ─┐
0003 ────────┼─→ 0006 → 0007 ─┬─→ 0008 ─┐
0004, 0005 ──┘                ├─→ 0009 ─┼─→ 0011 → 0012
                              └─→ 0010 ─┘
```

- 0001–0005 can run in parallel.
- 0006 also waits for the `NET` and `KRN` requests.
- 0008–0010 wait for `UPD`, `KRN` and the Effector server.
- 0012 needs the Effector server's task 75 (part A) and [issues-human](../issues-human/README.md#7-an-effector-server-for-mind-core).

## Acceptance criteria

- **In QEMU, on x86_64 and aarch64, against the test server (0002):**
  - the agent appears online, and is ready for operations;
  - it restarts a service the policy names and refuses one it does not;
  - it returns a log and a file from the policy's roots and refuses any other path;
  - it delivers a file with a checked hash;
  - it installs a signed package, updates it to a newer version, and refuses an unsigned or altered one;
  - it answers every shell action `denied`;
  - it survives a cut connection and a repeated command ID without running anything twice;
  - every operation appears in the audit trail.
- **Against the real Effector server, on a build named in the record (0012):** the same operations from the server's web interface and API, recorded with the configuration they ran on.
- **The authority map** ([docs/network/airlock.md](../docs/network/airlock.md)) has rows for both services, and each meets MC-11.11.
- **The profile:**
  - says which of these hold, on which configurations, and what is not done (MC-12.3);
  - names the agent in the threat model;
  - claims nothing from QEMU for a physical machine (MC-12.9).

## Related

[501](501-effector.md) (tests on real hardware, now through this agent), [351](351-self-update.md), 173 (the boot services' configuration; on the kernel track's branch until it merges), [550](550-network-on-real-hardware.md), [351-NET-0002](351-NET-0002-https-for-programs.md), [351-NET-0005](../issues-done/351-NET-0005-persistent-device-key.done), [109](../issues-done/109-session-parsers.done), [108](../issues-done/108-editable-network-policy.done), [351-UPD-0007](351-UPD-0007-updater-service.md), [351-UPD-0009](351-UPD-0009-rollback-policy-and-key-roles.md), [500](500-fuzzing-abi-and-idl.md), [docs/profile/threat-model.md](../docs/profile/threat-model.md).
