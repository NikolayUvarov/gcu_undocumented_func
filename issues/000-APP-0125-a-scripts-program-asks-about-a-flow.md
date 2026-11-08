# 000-APP-0125 — A program a script runs puts its question about a flow to the user

**Type:** tools (`shell`, `msh`) · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** — · **Constitution:** MC-3.11, MC-11.5

**Asked by:** NET, 2026-11-06, in [requests-APP.md](requests-APP.md), for [000-NET-0048](000-NET-0048-get-a-wget-and-curl-analog.md) (`get`). Numbered on 2026-11-08.

## Problem

`get` asks through its grant for a host the grant does not name (`socket.wit` 2.8). The shell's question (000-APP-0120) shows it when `get` is in front of a console. Started from a script (`msh -c "get …"`, `msh file`), it is not shown:
- `ShellHost::run` (`shell/src/msh.rs`) waits on the program with a loop of its own, which does not call `flowctl::tend`;
- so the broker refuses the question at 60 s, and `get` ends with 7.

The same holds for any program a script runs that asks.

## Plan

- The script's wait puts the question to the user as the shell's loop does: on the script's console, with the program's keys taken for the answer and given back.
- The answer's key is not read as the script's Esc or Ctrl+C.
- Until then `docs/network/downloads.md` says that a script's `get` is refused after 60 s (NET's text).

## Acceptance criteria

- **QEMU, the `net` suite:** a script's `get` to a host its grant does not name shows the question; N refuses it and O lets it through. The script goes on after either, and Esc still stops the script when no question shows.
- **Host tests** for the keys while a question shows in a script's wait.

## Related

[000-APP-0120](../issues-done/000-APP-0120-the-shell-asks-the-user-about-a-programs-flow.done), [000-NET-0048](000-NET-0048-get-a-wget-and-curl-analog.md).
