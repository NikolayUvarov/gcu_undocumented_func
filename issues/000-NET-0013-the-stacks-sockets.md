# 000-NET-0013 — The stack's sockets, listed for `netstat`

**Type:** network (`netstack`, interface) · **Owner:** `NET` track (worked by the storage session at the maintainer's request) · **Priority:** P3 · **Status:** open · **Blocked by:** — · **Main task:** — (for the tools track's [000-APP-0031](000-APP-0031-netstat.md)) · **Roadmap:** track D · **Constitution:** MC-11.6, MC-12.3, MC-12.4

From the tools track's request in requests-NET.md ("A list of the stack's sockets, for `netstat`", 2026-10-09).

## Problem

`socket.wit` names the interfaces (`interfaces`) and a grant's totals (`policy-usage`), and `netpolicy.wit` the live grants, but nothing names the stack's sockets. A `netstat` cannot show which connections are open, in what state, to where, or for which program.

## Plan

- `socket.wit` 2.4 adds `sockets(start)`: the open sockets from a table index, at most 32. Each comes with:
  - its handle, protocol and card, and its local port;
  - the remote address and port (0 for UDP);
  - its TCP state, as RFC 9293 names them;
  - the PID and badge of the client that opened it;
  - the bytes it moved through its client.
- Who may call it:
  - the operator (the shell's client) sees every socket;
  - a grant sees only its own, so a program never sees another's flows (MC-11.6);
  - the policy badge sees none.
- `netstack` counts what each socket moves, beside the grant's totals.
- `netcheck`, the network track's diagnostic, gains `open:` (a connection kept open) and `sockets` (the list for its grant), so the network track checks the list without the shell.

## Acceptance criteria

- In the `net` suite (x86, aarch64), `netcheck open:… sockets` lists its own connection as established, with the server's address and port, its badge and the bytes it sent.
- A program without a grant gets nothing.
- The operator's whole list is the tools track's to show (`netstat`, 000-APP-0031).
