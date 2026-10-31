# 110-NET-0052 — Listening TCP sockets for programs, and a listening grant

**Type:** network (`netstack`, `netpolicy`, `idl/socket.wit`) · **Owner:** `NET` · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [110](110-our-own-web-server.md) (its stage 2), for [112](112-our-own-ssh-server.md) too · **Roadmap:** track D (network) · **Constitution:** MC-5.1, MC-5.2, MC-10.3, MC-11.4, MC-11.6, MC-12.4

**Recorded by:** the kernel session, 2026-10-31, as the first step both servers need: the web server of 110's stage 2 and the SSH server of 112.

## Problem

- **MIND Core accepts no connection.** `netstack` gives a program outbound TCP and UDP through a flow grant. No program can listen, so no server can run on MIND Core.
- `socket.wit` knows the `listen` state of TCP's state machine. Nothing a program calls reaches it.
- **A flow grant is outbound only:** a program, a host, a port, a term, a volume. Nothing says who may connect to a program, from where, on which port.

## Plan

- **`socket.wit`'s next minor version, for a grant's badge:**
  - `tcp-listen(port, backlog)`: a listening socket on one port of the grant's;
  - `tcp-accept(socket)`: the next connection set up, with the peer's address and port, or `none-yet`;
  - an accepted socket is used as a connected one is, through the calls that exist, and charged to the same grant's volume.
- **A listening grant, `netpolicy`'s new line kind:** `listen <program> tcp <port> from <network>/<bits> [<seconds> [<bytes>]]`, at most 4 networks a port.
  - The stack answers a SYN to a port no grant listens on, or from a network outside the grant, with a reset.
  - The broker logs each grant given, and the stack each connection accepted: the program, the port, the peer (MC-10.3).
- **Limits** (MC-5.1, 5.2), each refusal named:
  - the backlog, at most 16;
  - half-open connections, a port's and in all, against a SYN flood;
  - accepted connections a grant holds at once.
- **No change to the ABI.** The interface is a minor version: every earlier call stays (MC-12.4).

## Acceptance criteria

- **Host tests:** the listening grant read and refused (ports, networks, limits); the backlog and the half-open limit.
- **QEMU, x86 and aarch64** (the `net` suite):
  - `netcheck listen:PORT` accepts a connection from the host through QEMU's forwarded port, echoes a line and is charged for it;
  - a port without a grant is refused;
  - a peer outside the grant's network is refused, with the stack's log line.
- **Docs:** `docs/network/policy.md`, `docs/idl`.

## Related

[110](110-our-own-web-server.md), [112](112-our-own-ssh-server.md), [101](../issues-done/101-network-stack.done), [102](../issues-done/102-network-policy-broker.done).
