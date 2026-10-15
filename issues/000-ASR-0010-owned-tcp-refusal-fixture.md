# 000-ASR-0010 — Own the closed TCP endpoint used by the network test

**Type:** assurance / QEMU fixture · **Owner:** ASTRA auditor-fixer (the maintainer's assignment, 2026-10-15) · **Priority:** P1 · **Status:** in progress · **Blocked by:** full gate · **Roadmap:** Assurance · **Constitution:** MC-12.1, MC-12.3, MC-12.9

## Problem

The network suite assumes that host loopback port 1 refuses TCP connections.
On the audit host it silently drops them instead: native connects to ports 1
and 9 time out. QEMU user networking therefore cannot deliver the TCP reset
needed to exercise MIND's `Refused` path. The full gate at `9a47d0dd` failed
this assertion after its DHCP, ping, DNS and HTTP checks had passed.

## Plan

Bind a loopback TCP socket to an allocated ephemeral port and keep it open
without listening during the request from MIND. This reserves the endpoint
against another process while the host rejects connections to it. Keep the
exact `FETCH: Refused` and `HTTPS: Refused` assertions and all other network assertions.

## Acceptance criteria

- The fixture owns the port until the guest command returns, then closes it
  on success or failure.
- The test still requires a refused connection; a timeout fails the test.
- A focused QEMU network run and the coordinated full gate pass. Results name
  their configuration and make no claim about physical NIC behavior. The TLS
  suite uses the same owned-endpoint contract and retains its exact refusal.

## Related

[550-NET-0031](550-NET-0031-bounded-driver-calls-and-ingress.md),
`tests/qemu_smoke.py::{net_suite,tls_suite}`.

## Investigation (2026-10-15)

In four-CPU x86 QEMU with the unchanged `9a47d0dd` image, the fixed port timed
out after 4.968 seconds; packet capture contains three outgoing SYNs and no
incoming reset. A held non-listening ephemeral socket returned `Refused`
twice, each in about 0.242 seconds. Captured SYN-to-RST/ACK delays were 469
and 357 microseconds. HTTP succeeded before and after, without a netstack
driver cooldown. The correction is to the test endpoint, not the TCP stack
or its expected behavior. The full gate remains pending.

The isolated complete four-CPU x86 network suite passes with this fixture,
including 30 MiB HTTP/TLS interruption/resume, policy, multiple cards and
driver/stack restart. The earlier full gate also reaches the same fixed-port
failure in its 6 GiB network group; both paths use the single corrected
fixture. Final-candidate full-gate evidence is recorded separately in
[the audit validation](../issues-audit/2026-10-15-astra-validation.md).

The final `1aea9100` host/x86 stage passed all 24 groups, including the
normal network suite on Ryzen and the 6 GiB suite on Intel i9. ARM was
explicitly deferred by the maintainer, so the full-gate acceptance criterion
remains open. These tests do not establish either physical NIC driver.

## TLS follow-up (2026-10-16)

Candidate `0c4205e77322c3f751d26990e139e677d8bb30e6` passed the fixture
build (31 s) and the boot/display/network checks on UN-LEGION9, but its main
QEMU group failed after 557 s at the TLS suite's remaining fixed-port
request. The shell timed out waiting for `HTTPS: Refused`; this is not a
passing group. A native loopback check on this same host observed port 1
time out after 1.000450 s, while the held non-listening ephemeral endpoint
returned ECONNREFUSED after 0.000582 s.

Apply the existing fixture fix to `tls_suite` as well, retaining the exact
`HTTPS: Refused` assertion and default guest timeout. No TLS or TCP source,
policy, certificate assertion or timing limit changes. The corrected focused
TLS run and subsequent integrated candidate results must be recorded
separately; earlier Ryzen/i9 results do not cover this failed local run.
