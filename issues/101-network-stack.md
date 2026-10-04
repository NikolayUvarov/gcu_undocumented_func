# 101 — Network stack service (IPv4, ARP, ICMP, UDP, TCP)

**Type:** service · **Owner:** network track · **Priority:** P2 · **Status:** open · **Blocked by:** 100 · **Roadmap:** track D (stage VII, Babel) · **Constitution:** MC-11.3–11.6, Appendix B.6 (network stack)

## Problem

Programs need flows (UDP sockets, TCP connections), not raw frames. Per B.6 the stack holds packet and flow endpoints with quotas and has no direct power over the device.

## Plan

- `netstack` service, the only client of `virtio_net` (issue 100). Candidate implementation: [smoltcp](https://github.com/smoltcp-rs/smoltcp) (`no_std`, 0BSD licence) behind a MIND IDL interface; record it in `THIRD_PARTY.md`.
- Configuration by DHCP, with a static fallback; ARP, ICMP echo, UDP, TCP; DNS resolver as a client library or a separate parser.
- Interface `idl/socket.wit`: open a flow from a capability, send and receive in bounded buffers, close; every flow is an endpoint with quotas (buffers, rate), counted per owner.
- Tools: `ping`, `nslookup`, a minimal HTTP fetch.

## Acceptance criteria

- In QEMU user networking: DHCP lease, `ping 10.0.2.2`, a UDP DNS query and a TCP connection to a host-forwarded port work in a QEMU suite.
- A killed stack is restarted by `init`; open flows fail with `ERR_PEER`, new ones work.

## Related

[100](100-virtio-net-driver.md), [102](102-network-policy-broker.md).
