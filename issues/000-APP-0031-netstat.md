# 000-APP-0031 — `netstat`: the open connections, their state and their programs

**Type:** tools · **Owner:** tools track (`APP`) · **Priority:** P3 · **Status:** open · **Blocked by:** a list of the stack's sockets ([requests-NET.md](requests-NET.md)) · **Main task:** — (the tools plan's phase T4, [docs/tools §5](../docs/tools/README.md)) · **Roadmap:** track D, G · **Constitution:** MC-11.6, MC-12.3

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

## Related

[requests-NET.md](requests-NET.md), [docs/tools §5](../docs/tools/README.md), [102](../issues-done/102-network-policy-broker.done) (grants).
