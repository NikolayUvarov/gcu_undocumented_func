# Effector agent development

The [contract](protocol.md) describes the native ConfigAgent wire subset and
the exact source/live server revisions reviewed. MIND's runtime agent is
still [750](../../issues/750-effector-agent.md); these files are not evidence
that a MIND machine is remotely manageable yet.

## The two services and what each holds

The agent is two boot services on the network path of Appendix B.6
([750](../../issues/750-effector-agent.md)). They talk only through
[`idl/effector.wit`](../../idl/effector.wit) 1.0, which `effector_gw` serves.
The grants below are the ones planned in
[750-KRN-0119](../../issues/750-KRN-0119-boot-images-and-grants-for-effector.md);
neither service runs yet, so this section is the design the code must keep,
not a description of running code (MC-12.3).

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
every request. This is the design, not yet running code.

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

- the heartbeat: release version, uptime, memory, the boot services' states,
  whether a request is running;
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

`effector` is the session service's first slice: a program started from the
shell until [750-KRN-0119](../../issues/750-KRN-0119-boot-images-and-grants-for-effector.md)
makes it a boot service. It heartbeats, polls and answers **every** command
`denied`, after journaling the answer; nothing on the machine is acted on yet
(the gateway is [750-EFF-0007](../../issues/750-EFF-0007-gateway-service.md)).

**Files, all in one directory** (the only one the shell lends it, `REQUEST_FILE`):

| File | Written by | Contents |
|---|---|---|
| `effector.conf` | the owner | `effector-config 1`, then `server https://HOST[:PORT][/PATH]`, one or two `pin leaf:HEX` or `pin spki:HEX` (current, next), and optionally `name` (the certificate's name, default the host), `address A.B.C.D` (needed when the host is a name), `hostname` (default `mindcore`). Any other line refuses the file (`effector/src/config.rs`) |
| `token` | the owner | the native AgentToken, one line; never printed. An operator API key is not a token |
| `agent_id` | the agent, once | `mind_` and 16 hex digits from the random source, or the owner's own (1–64 of `[A-Za-z0-9_-]`) |
| `journal` | the agent | `effector-journal 1`, then one line per answer, appended and flushed before its ACK (`effector/src/journal.rs`) |

**The network policy line** (`netpolicy add`, which asks the user, 108): the
program name `effector`, the server's address and port, a term and volume for
a long-running service, e.g. `effector 203.0.113.7 tcp 443 31536000 10000000000`.

**Run:** `effector data/effector/effector.conf [--for SECONDS]`. With `--for`
it stops after that long and sends a `shutting_down` heartbeat; without it, it
runs until it is stopped. It prints one line per event (`CONNECTED`, `DENIED`,
`ACK … ACCEPTED`, `REPEATED`, refusals); the token never reaches the log.

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

**As boot services** (2026-10-22, built, not yet started by `init`): `init`
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
| | `SLOT_SYSINFO` (10), `SLOT_LIFECYCLE` (11), `SLOT_LOG` (12) | memory, the services (750-KRN-0121's limited client), the log ring |

A grant `init` does not give makes the requests that need it fail; without
its private directory the gateway cannot write an audit line, so it refuses
every request (MC-10.6).

**Not done in this slice** (each stays in its task):

- `init` starting the two services, their grants and private directories
  (750-KRN-0119, 0120, 0121): until then the session runs from the shell and the
  token sits in `data/effector/`, which the shell can read;
- the session handing commands to the gateway (750-EFF-0006, 0007): every
  command is still denied;
- the wake-up stream (SSE): polling only, as the contract allows a first slice;
- journal retention: the journal keeps 64 answers and then refuses new commands;
- a `cancel_command` the server sends while an answer waits for its ACK (it
  may, even with `at_capacity=1`) is not answered and expires on the server;
  the command it names was already answered `denied`;
- the ten-minute online check beside stalled cards (550-NET-0032), and the
  device key as a client certificate (`connect-pins` cannot send one).

**Tested** in QEMU (`tests/qemu_smoke.py --suites effector`, x86_64 and
aarch64) against the fixture: online with heartbeats at most 1.6 s apart over
30 s; three commands denied after journaling; an ACK the server failed retried;
a shutdown heartbeat; restarted, three saved answers read back and a
redelivered command answered from the journal; a server with another
certificate refused before any request; the token absent from the log. Host
tests: `tests/effector_{session,json,wire,config,journal,policy}_host.rs`. This
is evidence for QEMU against the fixture, not for the real Effector server or a
physical machine (750-EFF-0012).
