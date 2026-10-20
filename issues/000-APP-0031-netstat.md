# 000-APP-0031 — `netstat`: the open connections, their state and their programs

**Type:** tools · **Owner:** tools track (`APP`) · **Priority:** P3 · **Status:** in progress (on the tools branch, 2026-10-20; the gate left) · **Blocked by:** — ([000-NET-0013](../issues-done/000-NET-0013-the-stacks-sockets.done), the list of the stack's sockets, done) · **Main task:** — (the tools plan's phase T4, [docs/tools §5](../docs/tools/README.md)) · **Roadmap:** track D, G · **Constitution:** MC-11.6, MC-12.3

## Problem

The tools plan's phase T4 names `netstat`, after track D. Today:

- `ip` shows the interfaces and their counters;
- `netgrants` shows the flow grants of the policy broker.

Nothing shows the connections themselves: which are open, in what TCP state, to which address and port, and for which program. `socket.wit` has no call that lists the stack's sockets.

## Plan

- A shell command `netstat` (the shell holds the stack's and the broker's clients):
  - one row per socket: protocol, local port, remote address and port, TCP state, bytes each way, and the program, found by mapping the socket's grant badge through `netpolicy.list`;
  - then the interfaces with their counters, as `ip` shows them;
  - `-i` for the interfaces alone.
- The same in `msh` under `requires: network`, and a `netstat` line for `console`'s built-in commands if it fits.
- Until the network track lists sockets, nothing here is built: grants and interfaces alone are already `netgrants` and `ip`.

## Acceptance criteria

- The net suite, x86 and aarch64: while `download` fetches from the test server, `netstat` shows its TCP connection to the server's address and port as `ESTABLISHED`, under the name `download`; after it ends, the row is gone.
- Host tests of the table's formatting.

## Progress (2026-10-20)

- **`netstat [-i]`** (`shell/src/net.rs`, `shell/src/netstat.rs`): one row per socket from `socket.wit` 2.4 `sockets` (every one for the operator, 32 a page): the protocol, the local port, the remote address and port, the TCP state as RFC 9293 writes it, the bytes each way and the program: the grant's program (`netpolicy` `list`, by the socket's badge), else the task's name by its PID, `shell` for the operator's own; then the count and the cards as `ip` shows them. `-i`: the cards alone.
- **`msh`:** under `requires: network`. **A client of the shell's commands** (`console` in `wm`): asked in the shell's window first, as `ping` is: it shows every program's connections.
- **Host test:** `tests/shell_host.rs`, the row lined up under the header, a UDP socket with no remote end, the states.
- **The `net` suite's download checks** (x86, aarch64): with both downloads connected to the held server, `netstat` shows two `ESTABLISHED` rows to its port under `download`; after they end, none; `netstat -i` shows the address.
- `console`'s built-in commands do not have it: it needs the stack's operator client, which only the shell holds.
- **Left:** the gate on this batch.

## Related

[requests-NET.md](requests-NET.md), [docs/tools §5](../docs/tools/README.md), [102](../issues-done/102-network-policy-broker.done) (grants).
