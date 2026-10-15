# Bounded session core — contract 1.0

`effector/src/session.rs` is production `no_std` code for
[750-EFF-0006](../../issues/750-EFF-0006-session-service.md). It currently has
no boot entry point, network executor or durable store. No MIND endpoint has
been brought online by this code. The core consumes typed, already validated
commands and emits scheduling attempts and a durable-write request.

## Boundaries

- Every action, including shell, service, file, update and cancel actions,
  receives an explicit `denied` result. An already expired command receives
  `expired`. There is no action dispatch callback or local-effect authority.
  A future gateway result must come through the separately authorized 0003/
  0007 interface; this internal denial journal is not a remotely callable
  operation that can claim another action completed.
- The command ID and agent ID use 1–64 ASCII letters, digits, `_` or `-`.
  Action, program and payload are at most 512 UTF-8 bytes. The fingerprint
  includes length-delimited fields, target agent and optional expiry under a
  versioned domain tag. A conflicting reuse of an ID fails before a write.
- The journal holds at most 64 records. `Store` requires an atomic, durable
  record write by the trusted journal adapter before `stored(..., true)`.
  Before that confirmation, no ACK is emitted. A failed/uncertain write blocks
  new commands and ACKs until recovery; heartbeat scheduling continues.
- Recovery supplies complete validated records, bound to the same agent.
  Duplicate IDs and over-capacity recovery fail. Saved results are re-ACKed,
  never re-executed. ACK success, permanent refusal and expiry retain records;
  this first slice never evicts them, so a full journal refuses new commands.
  The gateway must provide persistence, recovery validation and an explicitly
  reviewed retention/compaction policy before broader use.
- ACK is a native string-valued JSON result. The two possible inner JSON
  results are fixed constants; encoding the outer request belongs to the
  bounded writer and wire adapter. Only a schema-validated HTTP 200 `ok`
  counts as success. Network errors, 429 and 5xx retry; redirects and other
  responses do not supply a new origin or authorization.

## Scheduling and deadlines

Heartbeat, poll and ACK each have their own in-flight attempt. A delayed poll
or journal write does not suppress heartbeat scheduling. Heartbeat is due
at 1500 ms intervals; normal polling at 10 seconds or after a wake-up.
A wake-up during an active poll survives until its completion. Failed polling
retains its backoff even when further wake-ups arrive.

Each network attempt carries a 1500 ms deadline. The executor must use a
separate connection and independently advance/cancel each attempt. The core
reports an overdue attempt; it never treats a timeout as proof that the
transport stopped, and never starts a replacement while that attempt remains
active. Sequence numbers reject late completions. ACK retries start at five
seconds, grow exponentially and include bounded jitter supplied by the caller,
capped at sixty seconds. Permanent failures stop automatic attempts while
retaining evidence. An explicit redelivery can return the same saved result.

Command expiry is converted from a validated Unix timestamp to a monotonic
deadline once per delivery/recovery. Missing or unrepresentable time fails;
subsequent wall-clock changes do not extend the current retry deadline.
An expired first delivery may report `expired` once; retries past its term
stop without erasing the record. On restart, expired saved records remain
available for reconciliation. TLS still needs independently valid RTC time.

`stop` prevents new polling/commands and schedules a final `shutting_down`
heartbeat after any current heartbeat. Its accepted completion prevents
further heartbeats. The executor must set the outgoing heartbeat state from
`stopping()` when starting that attempt; transport failure does not become a
successful shutdown report.

These are scheduling properties, not a measured heartbeat latency guarantee.
`mind::http::Connection::request/read` are synchronous, so two connections in
one blocking loop are insufficient. A cooperative transport executor or a
separately scheduled authorized helper remains necessary. The hardware audit
also records the stack's aggregate per-card latency in 0006; a healthy link
beside stalled cards must be measured before online acceptance.

## Build and verification

Build the core without a host operating system:

```sh
cargo build --manifest-path effector/Cargo.toml --release --target x86_64-unknown-none
```

The host regression source includes the production core and SHA-256 code:

```sh
rustc --edition=2021 --test tests/effector_session_host.rs -o /tmp/effector-session-tests
/tmp/effector-session-tests
```

On 2026-10-15, implementation commit
`e947245086957218e3c3db92ddedb5dba767c0e4` built for
`x86_64-unknown-none`. After the coordinator published it in `fast-test`
`b8e02e3ab0e191d848c51c8a24742981cef52e46`, all 20 host regressions passed
on the implementation's own `codex/EFF-session` branch: zero failures or
ignored tests. Host: x86_64 Linux 6.6.87.2 WSL2; compiler:
`rustc 1.101.0-nightly (c36f14571 2026-10-01)`. No build or test ran on
`fast-test`. This is host state-machine evidence, not a QEMU, transport,
hardware or full-gate result.

Still open: the 0004 writer/schema adapter, actual HTTPS transport using
`connect-pins` and parser-service JSON, native configuration/private-token
setup, gateway durable store, kernel boot images and grants, owner controls,
SSE, pin rotation, the two-architecture QEMU/isolation suite, ten-minute online
behavior and live server/hardware acceptance. Existing 0006 and the KRN/APP/
UPD requests retain these requirements; this core does not close the task.
