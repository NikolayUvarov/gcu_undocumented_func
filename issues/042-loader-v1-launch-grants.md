# 042 — Loader v1: launch with granted capabilities, console programs

**Type:** architecture · **Priority:** P0 · **Status:** open · **Blocked by:** 038 · **Roadmap:** track G, tools plan F7 · **Constitution:** MC-3.3, MC-3.7, MC-3.11

## Problem

`loader` gives every application the same fixed grant set and always a screen. A tool cannot receive what it needs (a sysinfo client, a file handle), and granting by program name would break MC-3.7.

## Plan

- `idl/loader.wit`: `begin(request) -> session` (name and arguments in the buffer), `grant(session, slot, cap: borrow<endpoint>)` repeated, `commit(session, flags) -> pid`, `abort(session)`, `inspect(name)` → the program's request. Old `LOADER_RUN`/`LOADER_LIST` remain for compatibility until C8.
- The loader copies only capabilities the caller passed, into slots 7–9 (application-defined); standard slots 1–6 are unchanged.
- Request: ELF note `.note.mind.request` (`mind::request!` macro) with requested capabilities (`sysinfo`, `file`, `dir`, `lifecycle`, `log`) and `console` (no screen). It grants nothing.
- Shell as the user's agent: `top`, `memmap`, `load`, `hw` get a sysinfo client; `view`/`edit <file>` get a handle to that file; `fm` gets directory handles; console programs keep the shell in the foreground and their output is drawn in the shell.

## Acceptance criteria

- QEMU: a program started without a grant cannot reach `sysmon` (`ERR_INVALID` on its slot), the same program started by the shell can; a console program prints into the shell.

## Related

[docs/tools](../docs/tools/README.md) F7.
