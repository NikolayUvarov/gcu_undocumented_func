# 110-NET-0052 — Listening TCP sockets for programs, and a listening grant

**Type:** network (`netstack`, `netpolicy`, `idl/socket.wit`) · **Owner:** `NET` · **Priority:** P2 · **Status:** in progress · **Blocked by:** — · **Main task:** [110](110-our-own-web-server.md) (its stage 2), for [112](112-our-own-ssh-server.md) too · **Roadmap:** track D (network) · **Constitution:** MC-5.1, MC-5.2, MC-10.3, MC-11.4, MC-11.6, MC-12.4

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

## Progress

2026-11-04, the network track (01a74a85, bb8ac16b):

- **`socket.wit` 2.9** (generated code committed, `mind_idl.py --check` passes). Every earlier call stays.
  - **For a grant's badge:** `tcp-listen(port, backlog)` and `tcp-accept(listener)`, which answers the socket, the peer's address and its port.
  - **For the broker's badge alone:** `policy-listen(badge, rules)`, a grant's listening rules (a port and a network each).
- **`netpolicy`** reads `listen <program> tcp <port> from <network>/<bits> [<seconds> [<bytes>]]`.
  - At most 4 networks a port. A network with host bits set is refused, and so is a line given twice.
  - It logs each grant: `GRANT n TO prog: LISTENS ON TCP p FROM a.b.c.d/bits`.
- **`netstack`:**
  - **Limits:** a backlog of at most 16 sockets a port, and at most 32 listening sockets in all. A port already listened on is refused.
  - **Half-open connections** end after 10 s, which bounds a SYN flood to the backlog.
  - **A peer outside the grant's networks** is aborted (a reset) and logged: `PORT p REFUSED a.b.c.d:port: NOT A NETWORK ITS GRANT NAMES`.
  - **Each connection accepted is logged** with its peer, PID and badge.
  - **An accepted socket** is used and charged as a connected one, to the same grant.
  - **The grant's end** closes its listeners: `PORT p NO LONGER LISTENS`.
- **`netcheck listen:PORT:SECONDS`** echoes a line from each connection.
- **Host tests** (`tests/netpolicy_host.rs`): the line read and refused, and which addresses a network holds.
- **QEMU** (the `net` suite's `listen_check`), through QEMU's forwarded ports:
  - the host's connection to 7000 is accepted and its line echoed;
  - the 27 bytes pass the grant's 20, so the stack ends the grant, which shows the connection charged to it;
  - 7001, which no line names, is denied;
  - a peer outside 7002's 192.168.0.0/16 is reset and logged.
- **Runs:**
  - in this session's container (QEMU TCG, 4 CPUs): the check alone, then the group `x86: network, TLS and Effector` in full (2062 s);
  - on x32-128 (KVM) at 4469f706: `x86: network, TLS and Effector` (949 s) and `aarch64: network, TLS and Effector` (907 s), beside each other.
- **Docs:** `docs/network/policy.md` (the `listen` line, the listening grant and what is not provided), `docs/idl/README.md`.

**Not done:**
- Host tests of the backlog and the half-open limit. The stack's limits are checked in QEMU only through the ports above; no host test models them yet.
- The limit of accepted connections a grant holds at once is not separate. They count in the stack's sockets, as connected ones do.
- The half-open limit is the backlog, a port at a time. There is no limit of its own over all ports beyond the 32 listening sockets.
- A UDP server.

**Landed (2026-11-04).** Round 78's gate (candidate 6fc5b253, `main` e28cfe4d) passed `listen_check` in both network groups, x86 on x32-2 and aarch64 on pcu, and the IDL fuzzer with `socket.wit` 2.9's types. The host tests of the backlog and the half-open limit are still to come, so the issue stays open.

2026-11-04, the host tests of the backlog and the half-open limit:
- **`netstack/src/listen.rs`** holds a listening port's sockets, taken out of `main.rs` as they were, so that a host test runs the same code:
  - `Listener::new`, `turn` and `handle`;
  - `admit`: the port, the backlog and `LISTENING_MAX`.
- **`tests/listen_host.rs`** (5 tests, a `[[test]]` of `tests/network_budget`) runs it on smoltcp's own TCP. The wire is a loopback that can lose a client's acknowledgements. It checks that:
  - a backlog of two holds two connections, a third peer is reset, and one accepted leaves room for one more;
  - a peer outside the networks is reset, never ready, and the backlog is filled again;
  - two connections left half open hold the backlog, a third peer is reset, and they go between 9.5 and 10.5 s, after which the port listens again;
  - the bounds hold: port 0, a backlog of 0 or 17, a port taken, and the 32 listening sockets of every port together;
  - a listener that cannot listen leaves nothing in the set.
- **Each test fails when its rule is broken.** Three changes to `listen.rs` were tried, and each one failed one test:
  - no reset of a peer outside the networks;
  - no timeout of a half-open connection;
  - the ready connections not counted in the backlog.
- **Docs:** `docs/network/policy.md`, what is tested.

Every acceptance criterion is met once this lands; the "not done" points above stay as limits of the design.

## Related

[110](110-our-own-web-server.md), [112](112-our-own-ssh-server.md), [101](../issues-done/101-network-stack.done), [102](../issues-done/102-network-policy-broker.done).
