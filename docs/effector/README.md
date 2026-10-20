# Effector agent development

The [contract](protocol.md) describes the native ConfigAgent wire subset and
the exact source/live server revisions reviewed. MIND's runtime agent is
still [750](../../issues/750-effector-agent.md); these files are not evidence
that a MIND machine is remotely manageable yet.

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
[0003](../../issues/750-EFF-0003-interface-and-policy.md) and
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
- `idl/effector.wit`, `effector_gw/src/main.rs`: versioned typed boundary to
  the policy-owning gateway. Start with status and explicit denial; enable
  named logs and test-account msh scripts only with their own grants/tests.
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
