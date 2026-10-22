# Effector agent development

The [contract](protocol.md) describes the native ConfigAgent wire subset and
the exact source/live server revisions reviewed. MIND's runtime agent is
still [750](../../issues/750-effector-agent.md); these files are not evidence
that a MIND machine is remotely manageable yet.

## The two services and what each holds

The agent is two boot services on the network path of Appendix B.6
([750](../../issues/750-effector-agent.md)). They talk only through
[`idl/effector.wit`](../../idl/effector.wit) 1.1, which `effector_gw` serves.
The grants below are those of
[750-KRN-0119](../../issues/750-KRN-0119-boot-images-and-grants-for-effector.md),
0120, 0121 and 0153, and the table below "As boot services" says which slot
holds which. Those not built yet (the updater's client, the reboot badge, the
test account) are marked as later.

**Only in the development and maintenance modes** (the maintainer's decision
of 2026-10-21, [750-EFF-0013](../../issues/750-EFF-0013-development-and-maintenance-modes.md)).
The agent exists to manage every aspect of the OS during development. Its
authorities are therefore the widest any remote party gets. Outside these two
modes, `init` starts neither service and makes none of the grants below, so
there is no session, no flow and no gateway to attack. A mode is turned on
explicitly: by the operator, by a word written with the disk or by a command
confirmed at the local console, or by the Mind through the same switch when it
needs help from outside (MC-8.1, MC-12.6). It holds across reboots until it is
turned off. Inside a mode, the policy and the gateway's audit below still bound
every request. `init` does this since 750-KRN-0143.

| Service | Holds | Parses | Never holds |
|---|---|---|---|
| `effector`, the session | one `netpolicy` flow to the configured server; a `tls` client (`connect-pins`); a `parse` client; `rtc`; a read-only `vfs` view of `data/effector/` (the owner's configuration); its private directory `system/effector/` (the AgentToken, the agent ID, the ACK journal); the gateway's endpoint with the session badge | only HTTP framing: heads and JSON bodies go to `parse` (MC-11.11, [airlock](../network/airlock.md)) | any local authority: lifecycle, the log read badge, writable files outside its private directory, the updater, the reboot badge, the screen |
| `effector_gw`, the gateway | a lifecycle client limited to named services ([750-KRN-0121](../../issues/750-KRN-0121-a-lifecycle-client-for-named-services.md)); the log read badge; `sysinfo`; a `vfs` client writable in `data/effector/` (the audit) and `data/apps/` ([750-KRN-0120](../../issues/750-KRN-0120-writable-scopes-for-a-service.md)) and in the delivery root; read-only clients for the policy's roots and `log:`; a client of `updater` without the channel ([750-UPD-0020](../../issues/750-UPD-0020-the-gateways-client-of-the-updater.md)); `init`'s reboot badge; screen capture; later the test account's spawn right (501); its private directory `system/effector_gw/` | nothing from the network: only typed requests, checked by the generated decoder (MC-2.4), and the owner's [policy](policy.md) | the network, `tls`, `parse`, the device key, the AgentToken |
| the shell (the owner) | the gateway's endpoint with the owner badge: `reload-policy`, `audit`, `status` | — | — |

**If `effector` is taken over** (a parser or framing bug, a hostile answer it
mishandles): it can send the gateway any typed request with any origin. The
gateway still allows only what the policy names, audits it, and holds the
rest. What it gains is what the policy lets leave the machine, the session's
own token (so it can speak to the server as this agent) and the network flow
the policy line gives it.

**If the server, or whoever holds its key and the token, is hostile:** the
same as a taken-over `effector`, within the policy. The pinned certificate
keeps another server from posing as it; it does not make the server's
requests safe. Every request names its command, initiator and reason in the
audit before it runs.

**If `effector_gw` is taken over:** it holds the local authorities above, so
it is the agent's trusted part. Its exposure is small on purpose: no network,
no parser of outside bytes beyond the generated IDL decoder and the policy
file the owner wrote.

**What leaves the machine:**

- the heartbeat: the agent's ID, host name, architecture and version;
- telemetry every 30 s (750-EFF-0014): processor load, memory, uptime, the
  boot services' states, PIDs, processor time and memory, the volumes' space,
  the network cards' links and addresses, the count of commands answered;
- each request's result, bounded and without credentials;
- files, logs and screens the policy names, up to `max-bytes`.

The AgentToken, the device key, the journals, the policy and the audit never
leave it: the gateway refuses `data/effector/`, `data/apps/` and `system/` as
sources, and the session never reads them for the server.

**Recorded locally:** one audit line per request in `logd` and in a bounded
`data/effector/audit` (`effector_gw/src/audit.rs`), written before the reply;
if it cannot be written, the request is refused (MC-10.6).

## Local protocol fixture

Run its host tests from the repository root:

```sh
python3 tests/effector_server_test.py
```

Python 3.10 or later and the `openssl` executable are required. Python imports
only its standard library. OpenSSL generates a fresh, one-day test certificate
and key in a temporary directory; no private keys or third-party fixtures are
committed. Each server has a different certificate for wrong-pin tests. The
test uses TLS 1.3 and binds only `127.0.0.1`; sandboxed test runners must allow
loopback sockets. Linux was tested on 2026-10-15. macOS and Windows/MSYS2 are
not yet checked, and the QEMU agent suites do not exist yet.

The host controls the fixture through its Python object, not through a
network administration endpoint:

```python
from effector_server import EffectorFixture

with EffectorFixture() as server:
    command = server.queue("mind-test", "collect_logs", {"path": "log:/boot0001.log"})
    server.inject("/api/commands/ack", "ack_500")
    # Start the guest with server.url, server.leaf_pin and its test-only token.
    # Inspect server.operation(command), server.requests and server.uploads.
```

Import the module from `tests/` (as `qemu_smoke.py` does for its sibling
helpers). A standalone foreground fixture is also available:

```sh
python3 tests/effector_server.py --port 8443
```

It prints its base URL, certificate path and public leaf hash; its deliberately
public test token is `mind-effector-test-token`. Never reuse a production token
in this fixture. Interrupt the foreground process to close it and remove its
temporary key. QEMU user networking reaches host loopback as `10.0.2.2`; the
certificate has SANs for that address, `127.0.0.1` and `localhost`.

`queue`, `redeliver`, `operation`, `publish`, `requests`, `uploads` and
`deployment_results` are the control/observation interface. Faults consumed
once by a route are `delay` (0–5 seconds), `drop`, `redirect`, `ack_500`,
`unknown_fields`, `cut_stream` and `cut_download`. Queue an unknown action to
test denial. A second fixture supplies another certificate/key. Records keep
method, path, status and the fault name, never authorization headers or bodies.

Bounds: 16 concurrent sockets, five-second request I/O timeouts, eight agent
records, 128 commands/results, 16 published and 16 uploaded files (1 MiB each),
64 KiB JSON requests, 4096 recent request records. Hitting a command/file limit
fails explicitly. `requests` is a rolling observation window, not a durable
audit journal. The fixture models one execution slot and activation. It omits
production queue deadlines, reservations, artifact persistence, the installer,
operator API and native binary publication; they remain real-server checks.

The host suite covers real HTTPS framing/authentication, origin prefix,
connection reuse, heartbeat independent of a delayed poll, activation/busy
gates, unknown commands, string-valued ACK, ACK failure/retry, redelivery,
SSE wakeup/cut/EOF, file ownership/hash/cut, packages, redirects, another
certificate, strict JSON and size bounds. It tests the fixture, not an
as-yet-unwritten MIND client. No real endpoint is registered by these tests.

## Running the session (750-EFF-0006, first slice)

`effector` is the session service's first slice. Since
[750-KRN-0119](../../issues/750-KRN-0119-boot-images-and-grants-for-effector.md)
(ABI 16) it and `effector_gw` are boot images that `init` starts only in the
development or maintenance mode (`data/mode.conf`, 750-EFF-0013); no program
runs by their names. It heartbeats, polls, hands the actions below to
[`effector_gw`](../../issues/750-EFF-0007-gateway-service.md), whose owner's
policy decides each, answers any other command `denied`, journals each answer
before its ACK, and sends telemetry every 30 s.

**Files.** The owner's are in `data/effector/`, which the session reads through a read-only scope; its own are in
its private `system/effector`:

| File | Written by | Contents |
|---|---|---|
| `effector.conf` | the owner | `effector-config 1`, then `server https://HOST[:PORT][/PATH]`, one or two `pin leaf:HEX` or `pin spki:HEX` (current, next), and optionally `name` (the certificate's name, default the host), `address A.B.C.D` (needed when the host is a name), `hostname` (default `mindcore`), and `token none` for a server that asks no agent token. Any other line refuses the file (`effector/src/config.rs`) |
| `token` | the owner | the native AgentToken, one line; never printed. An operator API key is not a token. Not read when `effector.conf` says `token none` (contract 1.1: a server with `security.mode=disabled`) |
| `token` → `system/effector/token` | the agent | moved from `data/effector/token` at the first start; the owner may then remove the copy |
| `agent_id` | the agent, once, in `system/effector` | `mind_` and 16 hex digits from the random source, or the owner's own (1–64 of `[A-Za-z0-9_-]`) |
| `journal` | the agent, in `system/effector` | `effector-journal 1`, then one line per answer, appended and flushed before its ACK, and a `done` line once it needs no more ACK (`effector/src/journal.rs`); compacted at each start through `journal.new` |
| `artifact` | the agent, in `system/effector` | the screenshot or log of the answer waiting for its ACK, after a line naming its command, size and SHA-256; removed once acknowledged |
| `executed` | the agent, in `system/effector` | the count of commands answered, kept across restarts for telemetry |

**The network policy line** (`netpolicy add`, which asks the user, 108): the
program name `effector`, the server's address and port, a term and volume for
a long-running service, e.g. `effector 203.0.113.7 tcp 443 31536000 10000000000`.
- **The grant lasts until the next boot.** `init` takes the grant once at the first start and keeps it for every restart (MC-3.13). When its term or volume runs out, the session is refused (`Denied`) until the machine restarts. So the line's term and volume must cover the whole time the mode is on.
- **Several lines for `effector` share the smallest term and volume of them** (`docs/network/policy.md`).

**Run.** The owner writes `development` or `maintenance` in `data/mode.conf` and restarts the machine; `init` starts
`effector_gw`, then `effector --service`. `svc restart effector` restarts the session; `rm data/mode.conf` and a
restart end it. It prints one line per event (`CONNECTED`, `DENIED`, `ACK … ACCEPTED`, `REPEATED`, refusals), read
with `dmesg -s effector`; the token never reaches the log. A stop by `init` sends no `shutting_down` heartbeat yet.

**Against a real server** (750-EFF-0012): `MIND_EFFECTOR_LIVE=DIR tests/qemu_smoke.py --suites effector-live` puts
`DIR`'s `effector.conf`, `token` (if any) and `agent_id` on the guest's disk with the maintenance mode and the network
policy line for the configured address, and watches the session for `MIND_EFFECTOR_LIVE_SECONDS` (default 120). It
is not in CI.

**How it behaves:**

- One HTTPS connection to the pinned server (`tls.wit` 1.2 `connect-pins`),
  reconnected with back-off from 1 to 30 s. Every request is bounded by 3 s
  of silence, so a stalled server delays a heartbeat by at most that.
- Heartbeat every 1.5 s, before anything else in the loop; poll at least every
  10 s, at once again after a command or an accepted ACK (the server hands one
  command a poll), with `at_capacity=1` while an answer waits for its ACK, so
  the server holds new commands instead of delivering one that would be
  refused: it delivers each command only once.
- A command is journaled with its answer, then the answer is sent; an ACK the
  server failed is retried with back-off; after a restart the saved answers are
  sent again, never run again, and a redelivered command gets its saved answer.

**As boot services** (2026-10-22; `init` starts them since 750-KRN-0119): `init`
starts both with `--service` in the development or maintenance mode
(750-KRN-0119, 0143). Started any other way, `effector_gw` serves nothing and
says so. The slots and badges are one table in `libmind/src/effector.rs`
(`mind::effector`):

| Service | Slot | Holds |
|---|---|---|
| `effector --service` | `SLOT_RTC` (2), `SLOT_TLS` (20), `SLOT_PARSE` (28), `SLOT_NETWORK` (18) | as the program |
| | `SLOT_FILE` (7) | a reading client scoped to `data/effector/`: `effector.conf` and the token the owner leaves |
| | `SLOT_VFS` (3) | its private `system/effector`: the token (moved there at the first start), `agent_id`, `journal` |
| | `SLOT_GATEWAY` (4) | its client of `effector_gw`, badged `BADGE_SESSION` (1) |
| `effector_gw --service` | `SLOT_SERVICE` (1) | its endpoint: `BADGE_SESSION` for actions, `BADGE_OWNER` (2) for the shell's `status`, `reload-policy`, `audit`; any other badge is refused |
| | `SLOT_READER` (5) | a reading client of the volumes: `data/effector/policy`, the policy's file roots, `log:` |
| | `SLOT_FILE` (7) | the writable scopes of 750-KRN-0120, for deliveries |
| | `SLOT_VFS` (3) | its private `system/effector_gw`: the audit file (`audit`, then `audit.old` beyond 256 KiB) |
| | `SLOT_SYSINFO` (10), `SLOT_LIFECYCLE` (11), `SLOT_LOG` (12) | the counters, the services (750-KRN-0121's limited client), the log ring |
| | `SLOT_DISPLAY` (15) | the compositor's client (750-KRN-0153): the screen, which the compositor marks with its red dot while it is captured |

A grant `init` does not give makes the requests that need it fail; without
its private directory the gateway cannot write an audit line, so it refuses
every request (MC-10.6).

**Not done in this slice** (each stays in its task):

- packages, the system update, reboot and the test account's scripts
  (750-EFF-0008, 0009, 0010, 501): those commands are still denied;
- a `shutting_down` heartbeat when `init` stops the session;
- the wake-up stream (SSE): polling only, as the contract allows a first slice;
- the heartbeat's `programs` list is empty: the server's MIND programs come from
  its `programs-mindcore` configuration;
- a `cancel_command` the server sends while an answer waits for its ACK (it
  may, even with `at_capacity=1`) is not answered and expires on the server;
  the command it names was already answered `denied`;
- the ten-minute online check beside stalled cards (550-NET-0032), and the
  device key as a client certificate (`connect-pins` cannot send one).

**`collect_file`** (2026-10-22) goes through the gateway:
- **The order.** The session parses the command's payload (`path`, `max_bytes`) and asks `effector_gw` to open the file. The owner's `collect` roots and `max-bytes` decide, and the gateway audits.
- **The upload.** The session uploads the bytes, read from the gateway in 32 KiB pieces, to `/api/commands/file` with their SHA-256. A 201 makes the answer `completed`.
- **The answer.** A refusal or failure is `failed` with the gateway's reason (`denied`, `no-policy`, `bounds`, `upload`, …).
- **Run once.** A command runs once, only if it was never saved before. A redelivery gets the saved answer.
- **The journal** holds `completed` and `failed-<reason>` beside `denied` and `expired`.
- **Logs** come from `log:` paths (`log:/bootNNNN.log`) through the same action, as the server's MIND boundary expects.

**The other actions** (2026-10-22, contract 1.2), each decided by the owner's policy and audited by the gateway:
- **`screenshot`** (`screen on`): the gateway takes the compositor's copy of the screen, shrinks it to fit 1280x800 and
  writes a PNG (`effector_gw/src/png.rs`); the agent keeps it in `system/effector/artifact` and sends it inline in the
  ACK as base64, as the server's other agents do.
- **`collect_logs`** (`log ring`, or a `collect` root): the system log or a file, inline as text the same way.
- **`deliver_file`** (`deliver ROOT`): downloaded from the server straight into the gateway, which checks the size and
  SHA-256 before the file takes its name under the root (750-KRN-0120's scope).
- **`start_service`, `stop_service`, `restart_service`** (`service NAME ACTION`): the program the server names is the boot
  service; `init` checks the owner's policy again (750-KRN-0121).
- **An action runs once**, for a new command that has not expired and whose answer the journal has room for.

**Telemetry** (750-EFF-0014): every 30 s, a schema 1 sample to `/api/telemetry` (`effector/src/telemetry.rs`), from the
gateway's `sample` (`effector.wit` 1.1) and the session's view of the network cards. Rates (processor load, each
service's processor time) need two samples of the same boot and process, so the first sample has none; a value MIND
does not measure (interface byte counters, swap, disk I/O) is left out, never sent as zero. The first send and each
change in the server's answer give one line.

**Tested** in QEMU (`tests/qemu_smoke.py --suites effector`, x86_64 and
aarch64) against the fixture, the session started by `init` in the development
mode: online with heartbeats at most 1.6 s apart; two commands denied after
journaling; an ACK the server failed retried; the token kept in
`system/effector`; restarted by `svc`, the saved answers read back and a
redelivered command answered from the journal; with the configuration of a
server with another certificate, refused before any request; the token absent
from the log; with a policy collecting `data/reports/`, a 50 000-byte file
uploaded whole with its SHA-256 and answered `completed`, a file outside it
answered `failed: denied` and not sent, both in the gateway's audit; a
1280x800 screenshot sent as a PNG that zlib reads whole, its capture audited;
the system log sent inline; a 68 000-byte delivery written under the policy's
root and one outside it refused; sysmon restarted; telemetry samples that the
fixture's copy of the server's checks accepts, with processor load, 47 services
and the guest's address; restarted, 10 saved answers read back with none
acknowledged again. Host tests:
`tests/effector_{session,json,wire,config,journal,policy,gateway,png,telemetry}_host.rs`. This
is evidence for QEMU against the fixture, not for the real Effector server or a
physical machine (750-EFF-0012).
