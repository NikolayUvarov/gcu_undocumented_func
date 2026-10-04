# 103 — TLS service with non-exportable keys

**Type:** service · **Owner:** network track · **Priority:** P3 · **Status:** open · **Blocked by:** 101, 102 · **Roadmap:** track D · **Constitution:** MC-11.9, Appendix B.6 (TLS service)

## Problem

Secure connections need TLS; MC-11.9 separates access to key material from the right to request a key operation.

## Plan

- `tls` service on top of a flow capability: TLS 1.3 client first (candidate: rustls with a `no_std` crypto provider, licence to be checked), certificate validation against a root store from the boot volume.
- Private keys live in a key service; parsers and applications get a handle for an operation with a purpose and a budget, never the key.
- A session parser (for example HTTP) gets only bounded bytes in and typed messages out.

## Acceptance criteria

- An HTTPS request to a host-forwarded test server succeeds in QEMU with certificate validation; a wrong certificate is refused; no process except the key service can read the private key.

## Related

[101](../issues-done/101-network-stack.done), [102](102-network-policy-broker.md).
