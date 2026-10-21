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

## Next persistent agent slice

Use existing [750-EFF-0006](../../issues/750-EFF-0006-session-service.md),
[0003](../../issues-done/750-EFF-0003-interface-and-policy.done) and
[0007](../../issues/750-EFF-0007-gateway-service.md), not another protocol:

- `effector/src/main.rs` and `Cargo.toml`: an opt-in, persistent outbound
  session using `mind::http::Connection`, `tls.connect-pins`, `parse.json` and
  the network grant. Keep heartbeat and request execution independently
  schedulable; reconnect with backoff and report grant expiration distinctly.
- `effector/src/protocol.rs`: bounded JSON writer, typed responses with strict
  expected-field/type/duplicate checks, base-path preservation and native
  string-valued ACK. Refuse payloads beyond the current parser bounds.
- `effector/src/journal.rs`: persist command/result before ACK and recover
  pending ACKs after restart. No local side effects in this service.
- `effector_gw/src/main.rs`: the gateway serving [`idl/effector.wit`](../../idl/effector.wit)
  1.0 with the [policy](policy.md) and audit line of 750-EFF-0003. Start with
  status and explicit denial; enable named logs and test-account msh scripts
  only with their own grants/tests.
- Kernel-owned boot image entries and `init` grants, plus the tools-owned
  `effector setup/status/enable/disable`, remain tracked in their existing
  requests. AgentToken must be provisioned privately; an operator API key
  cannot replace it.
- `tests/effector_host.rs`: writer and response-schema edge cases, journal
  replay and conflicting IDs. `tests/qemu_smoke.py` effector suite: ten-minute
  heartbeat, wrong pin, interrupted stream/connection, repeat ID, failed ACK,
  unauthorized action and service grants. Run x86 and aarch64 separately.

Only then run [750-EFF-0012](../../issues/750-EFF-0012-run-against-the-effector-server.md)
against the named live server, then each real machine. ZIP/install/update work
is not a prerequisite to the first heartbeat, but remains part of main task
750 and must not be silently dropped from it.
