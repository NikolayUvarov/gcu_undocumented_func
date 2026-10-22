# MIND Core's Effector agent contract 1.2

**Status:** implementation contract, 2026-10-15; 1.1 and 1.2 on 2026-10-22 (below). No real-machine compatibility is claimed.
**Task:** [750-EFF-0001](../../issues/750-EFF-0001-protocol-contract.md).
Publishing this subset was approved by the maintainer on 2026-10-09
([owner decision](../../issues-human/README.md#7-an-effector-server-for-mind-core)).

## Sources and scope

- ConfigAgent source: `sst-test-deploy` commit
  `a29dd0086bf278e3e913f9b5203a70c96a4abcec`, principally
  `server/agent_protocol.md`, `server/openapi.json`, `server/api.go`,
  `server/commands.go`, `server/operations.go` and `agent/linux/worker.go`.
- The published [native protocol](https://hesere.uz/effector/agent-protocol)
  and [server identity](https://hesere.uz/effector/api/version) were read on
  2026-10-15. The latter reported `1.2.0.214`, build `214`, commit
  `a65552484d69`, build time `2026-10-15T11:25:45Z`.
- The source revision and deployed revision differ. Public documentation GETs
  establish neither authenticated interoperability nor support for MIND's
  operations. [750-EFF-0012](../../issues/750-EFF-0012-run-against-the-effector-server.md)
  records those checks when the agent exists.

**1.1 (2026-10-22):** `token none` in `effector.conf`, for a server that asks no agent token. Nothing else
changed; a configuration without that line behaves as under 1.0.

**1.2 (2026-10-22):** the actions the gateway serves, on routes and fields the reviewed server already has
(sst-test-deploy `server/artifacts.go`, `server/file_transfer.go`, `server/telemetry_store.go`, `docs/monitoring.md`).
Everything 1.1 did stays.

- **Every request** carries `X-Effector-Agent-ID: ID`. The server counts the agent's traffic by it; it is not a
  credential.
- **`screenshot` and `collect_logs` answer with an artifact inline**, as the server's other agents send screenshots:
  the result gains `"artifact":{"name","content_type","encoding":"base64","size","sha256","metadata"?,"data"}`, `data`
  last. A screenshot is `image/png`, at most 1280x800 and 2 MiB, with `metadata` `width` and `height`; a log is
  `text/plain; charset=utf-8`, `truncated` when the policy cut it. An artifact above 8 MiB is refused (`failed: bounds`),
  so the ACK stays below the server's 12 MiB.
- **`collect_logs`** takes `path` (JSON, or the payload itself): `ring` (the system log) and `boot` (the last boot's
  log) are logs, any other path is a file under the policy's `collect` roots.
- **`deliver_file`** takes `path`, `size`, `sha256` (64 lower-case hex digits), `overwrite` and `max_bytes`; the bytes
  come from `GET /api/commands/file`; the answer is `completed` once the gateway has checked them and named the file.
- **`start_service`, `stop_service`, `restart_service`**: the command's `program_name` is a boot service's name.
- **Telemetry**: `POST /api/telemetry`, schema 1, every 30 s, the server's `204` expected. Counters only: processors,
  memory, uptime, the boot services' processes, the volumes, the network cards, and the commands answered (this run
  and in all, kept in `system/effector/executed`).
- **The journal** adds a line `ID IDENTITY done -` once an answer needs no more ACK, so a restart does not acknowledge it
  again. When full, it drops its oldest record that is acknowledged and past its command's expiry; unacknowledged work
  is still never forgotten, and an action runs only when its answer has room.

`effector-agent 1.0` is the name of this MIND subset, not a version field that
the existing server negotiates. The server publishes a build identity and an
OpenAPI description, not an `effector-agent` handshake. Before deployment the
owner records the server build reviewed against this contract. Unreviewed
protocol changes require a new contract version and transition; they must not
be silently interpreted as the old contract.

The local fixture [tests/effector_server.py](../../tests/effector_server.py)
implements the wire exchanges below for host and future QEMU tests. It is a
test double with explicit fault injection, not an alternative fleet server.

## Connection, identity and trust

The owner sets one base URL, for example `https://hesere.uz/effector`, with no
trailing slash, a native **AgentToken**, and current/next SHA-256 leaf
certificate pins. Append `/api/...` to the entire base path; dropping
`/effector` talks to the wrong application. URLs supplied by the server must
remain at the configured HTTPS origin and within the configured base path.

Every native agent request carries `Authorization: Bearer <AgentToken>`, unless the owner's configuration says
`token none` (1.1). That is for a server whose `security.mode` is `disabled`, as `hesere.uz` runs on 2026-10-22: it
checks no agent token, so the agent reads no token file and sends no `Authorization` header. A missing token file
never means `token none`; it stops the agent. The pinned TLS connection is the same in both cases. Without the
token, the server cannot tell this agent from anyone who reaches it, but the agent still knows its server by the pin.
Operator `API_KEY` credentials and `/api/v1/operations` belong to the operator
side and are not agent credentials or routes. Never send credentials over
HTTP, follow a redirect, or log a token. Setup must move the token into the
agent's private directory; command arguments and public configuration are not
secret storage.

The server's leaf DER hash must match either configured pin. Check its name,
validity, server purpose and constraints as well as the TLS handshake. The
existing ConfigAgent accepts a pinned leaf as a replacement for an unknown
root, while still validating those properties. MIND's TLS service uses TLS
1.3; an endpoint allowing only TLS 1.2 is incompatible. A rejected pin, name,
certificate time or redirect is an error, not permission to downgrade.

Generate `agent_id` once with at least 64 random bits, store it durably, and
use 1–64 ASCII characters from `[A-Za-z0-9_-]` in this subset. The server
accepts up to 128 bytes; the narrower client grammar makes query construction
unambiguous. Identity is `os_version: "mindcore"`, `arch: "x64"` or `"arm64"`.
Do not advertise Linux or Windows to get their configuration or binaries.

The release version goes in `agent_version`. Device-key fingerprint and image
manifest hash are required run evidence for 750/501, but the reviewed native
heartbeat schema has no fields for them. Do not invent a guarantee that the
server persists unknown fields: agree an extension first and bind the run to
that evidence in the meantime. Likewise `connect-pins` currently cannot send
a device client certificate; token authentication is not device-key identity.

## Routes

JSON is UTF-8 with `Content-Type: application/json`. A request with a body has
an exact `Content-Length`. HTTP/1.1 responses may be length-delimited or
chunked. Only the SSE route intentionally remains open. Unknown object fields
are ignored within the declared bounds; duplicate keys and invalid types are
refused. Missing required fields never acquire permissive defaults.

| Method and suffix | Request | Successful response and interpretation |
| --- | --- | --- |
| GET `/api/config?platform=mindcore-x64` or `mindcore-arm64` | Optional repeated `capability` parameters for implemented features only | `200 {"programs":[]}` or platform-specific definitions. Never consume another OS's program definitions. |
| GET `/api/agent_config?agent_id=ID` | Also `X-Effector-Agent-ID: ID` | `200` runtime settings. The local URL, token and authorization policy cannot be widened by a response. |
| GET `/api/agent_version?platform=PLATFORM` | Also `X-Effector-Agent-ID: ID` | `200` version metadata; unknown or unsupported platform is not an update for MIND. Until signed MIND updates exist, report unsupported and download no replacement executable. |
| POST `/api/heartbeat` | Identity and state below | `200 {"status":"ok"}` registers/refreshes the endpoint. There is no separate registration route. |
| GET `/api/commands/poll?agent_id=ID` | Add `at_capacity=1` while busy | `200 {"commands":[...],"parallel_limit":1}`; activation or update gates may return no commands. |
| GET `/api/commands/stream?agent_id=ID` | `Accept: text/event-stream` | `200`, an SSE wake-up stream; fetch actual commands through poll. |
| POST `/api/commands/ack` | `command_id` and **string** `result` | `200 {"status":"ok"}` accepts the saved result. |
| GET `/api/commands/file?agent_id=ID&command_id=CMD` | A sent `deliver_file` command | `200` bytes, checked against the command's size and SHA-256. |
| POST `/api/commands/file?agent_id=ID&command_id=CMD&sha256=HEX` | A sent `collect_file` command; raw bytes, not JSON | `201` artifact metadata, then ACK the operation. |
| GET `/api/deployment/download/PACKAGE` | A permitted `deploy` command | `200` package bytes. Verify the signed MIND package inside it before installation. |
| POST `/api/deployment/result` | Agent/package/operation, exit code, bounded output, timestamps and `success` | `200 {"status":"ok"}`; report the command result through ACK too. |
| POST `/api/telemetry` | `X-Effector-Agent-ID: ID`, schema 1 samples | `204`, no body; optional after the basic agent works. |

Commands and file transfers require activation when the server enables that
policy. `activation_required:true` means awaiting owner activation, not a
broken connection. Configuration must not automatically enable MIND actions
just because the server supplied them.

## Heartbeat and scheduling

A minimal heartbeat is:

```json
{"agent_id":"mind_test_0123456789abcdef","hostname":"mind-test","os_version":"mindcore","arch":"x64","run_mode":"service","username":"effector","agent_version":"0.1.0","programs":[],"system_state":"running","busy":false,"max_concurrent_commands":1,"occupied_queues":0,"free_slots":1,"config_auto_update_enabled":false,"transport_mode":"https_only","transport":"https"}
```

Send it every 1–2 seconds independently of command execution, SSE and large
transfers. The reviewed server marks an endpoint offline after **5 seconds**
without a heartbeat (`server/agent_registry.go`). Advertising `busy` must not
stop heartbeat. While executing, include `busy_since` (RFC3339),
`executing_commands`, `occupied_queues:1` and `free_slots:0`; on an orderly stop
send `system_state:"shutting_down"`. Invalid RTC time must be diagnosed; it
must not be hidden by disabling certificate-time checks.

Use a separate connection for SSE. Its initial bytes are `: connected\n\n`;
a wake-up is `event: command\ndata: ready\n\n`. The reviewed server closes the
stream within 55 seconds. Reopen after EOF and poll after wake-up and at least
every ten seconds. Network failure reconnects with capped exponential backoff
and jitter; unrelated local work keeps running. A long test must never run on
the loop that sends heartbeats. The first transport slice may use polling
alone, explicitly leaving the SSE acceptance criterion open.

## Commands, results and delivery

The native command has `id`, `agent_id`, `program_name`, `action` and
`new_hostport`. The last field is a historical name: it contains a string,
often itself serialized JSON. State/time fields may also occur. Reject a
mismatched agent ID, malformed payload, expired command, duplicate keys or
unsupported required field before dispatch to the local gateway.

| Action | Payload and intended MIND operation | Until its gateway/policy exists |
| --- | --- | --- |
| `start_service`, `stop_service`, `restart_service` | Configured program maps to an allowlisted service | `denied` |
| `collect_logs` | Path or JSON `path`, `timeout_seconds`; named permitted log source | `denied` |
| `collect_file`, `deliver_file` | JSON path, allowed roots, maximum bytes, deadline; delivery adds artifact ID, size, SHA-256, overwrite | `denied` |
| `exec_v1` | JSON `command`, `run_as`, `session`, `timeout_seconds`; only a test-account msh invocation with declared and permitted grants (501) | `denied` |
| `screenshot` | Empty payload; policy-authorized native capture | `denied` |
| `deploy` | JSON `operation`, `package_id`; only a verified MIND application package | `denied` |
| `update_agent` | Ask the existing signed system updater; never replace a service with a Windows/Linux ConfigAgent binary | `denied` |
| `refresh_config`, `update_agent_config` | Re-read owner-authorized settings; never widen policy or replace origin/token | `denied` until implemented |
| `cancel_command` | The ID of a queued/interruptible request | Explicit result; receipt alone does not establish execution stopped |
| `exec`, `powershell_v1`, `console_v1`, `console_users_v1`, `collect_registration`, `set_hostport`, `uninstall_agent`, `performance_measure`, unknown actions | No permitted mapping in contract 1.0 | `denied` |

An unsupported action must receive a result; silently discarding it leaves the
server waiting. Denial is a typed result, for example this ACK (note the
double JSON encoding):

```json
{"command_id":"cmd-1","result":"{\"status\":\"denied\",\"exit_code\":1,\"stdout\":\"\",\"stderr\":\"\",\"truncated\":false,\"run_as\":\"effector\",\"error\":\"action_not_allowed\"}"}
```

Supported final statuses are `completed`, `failed`, `timeout`, `expired`,
`cancelled`, `rollback`, `run_as_unavailable`,
`interactive_session_unavailable` and `denied`. The reviewed server changes
`completed` with nonzero exit code to `failed`. Do not use ambiguous plain-text
results for this subset. A timeout after dispatch means outcome unknown until
reconciled; it is not evidence that no action occurred.

Journal the command ID, relevant request identity and completed result before
ACK. Retry an unacknowledged result at intervals of at least five seconds,
with backoff, and return the saved result for redelivery; do not re-execute it.
Conflicting reuse of an ID is an error. A permanent ACK rejection retains the
journal entry for diagnosis/reconciliation instead of making it executable
again. Command expiry must not erase evidence of a possibly completed action.

## Bounds and errors

| Item | Contract 1.0 client limit or requirement |
| --- | --- |
| HTTP response head | 8192 bytes, using the existing HTTP/parser service limit |
| JSON response body | 16384 bytes; reject overflow before parser IPC |
| JSON structure | 1024 values, depth 16, keys 64 bytes, decoded strings 512 bytes, duplicate keys rejected |
| Work in flight | One command, independently scheduled heartbeat |
| File/package size | Owner-policy bound checked before allocation or download; never more than server's 1 GiB |
| Journal | Bounded and durable; when full, refuse new dispatch rather than forgetting unacknowledged work |
| Output | Bounded, with explicit `truncated`; never include credentials or private journals |

The current JSON limits are a real integration limitation: `exec_v1` embeds
its JSON in one string, while the server can accept commands up to 32768
bytes. The initial agent must refuse a payload beyond 512 decoded bytes;
larger test scripts need a versioned parser extension or a delivery/reference
protocol. The server's 12 MiB ACK ceiling is not a reason to allocate that much
in MIND. The local fixture deliberately caps all JSON bodies at 64 KiB and
binary fixtures at 1 MiB per file; those are test-fixture limits, not claims
about the production server.

Expected errors are JSON objects with `error` and sometimes `code`. Retry
network failures, 429 and 5xx with backoff; do not follow 3xx. 400/401/403/404/
405/409 require diagnosis or changed configuration. In particular distinguish
bad token (401), denied scope/activation (403 or gated poll), unknown route/
platform (404), malformed payload (400) and state conflict (409). Close or
drain a response before reusing its connection.

## Compatibility work still needed

At the reviewed source revision, `programsForPlatform` falls back to the
Windows list for `mindcore`, file transfer chooses Windows paths for every
non-Linux platform, and native version/download routing has no MIND platform.
Add explicit MIND configuration and path/capability rules in the ConfigAgent
project; do not work around them by lying about the OS. The historical
external task 75 now concerns console credentials; the current MIND-specific
records are the task paths below.

The 2026-10-15 audit prepared external task
`issues-done/81-mindcore-server-platform-boundary.task.md.done` on an isolated ConfigAgent
`ASTRA` branch from `a29dd0086bf278e3e913f9b5203a70c96a4abcec`: separate MIND
programs, explicit MIND path roots and refusal of generic binary metadata.
Its server tests pass locally; the change has not been deployed. Active
external follow-up `issues/81-mindcore-01-agent-and-live-acceptance.task.md`
records release and real-agent checks. The separate open requirement
`issues/81-mindcore-02-package-metadata.task.md` covers exact package
size/hash binding and MIND architecture/non-shell entries with
[750-EFF-0008](../../issues/750-EFF-0008-application-packages.md); neither
task 81 nor 81-01 establishes signed-package support. These prepared changes
do not change the observed live-server compatibility statement above.

The next MIND slice is a persistent token-authenticated session, not a port of
the Linux Go binary: that binary requires Linux processes, files and systemd.
Use `libmind::http::Connection`, `tls.connect-pins` and `parse.json`; keep the
local operations in the separately authorized gateway. Existing tasks
[0003](../../issues-done/750-EFF-0003-interface-and-policy.done),
[0006](../../issues/750-EFF-0006-session-service.md),
[0007](../../issues/750-EFF-0007-gateway-service.md) and
[501](../../issues/501-effector.md) retain their remaining acceptance criteria.
