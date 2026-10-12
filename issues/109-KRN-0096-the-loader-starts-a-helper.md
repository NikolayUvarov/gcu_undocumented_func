# 109-KRN-0096 — The loader starts a helper with nothing but its caller's endpoint

**Type:** `loader`, `idl/loader.wit` 1.8 · **Owner:** kernel session · **Priority:** P3 · **Status:** in progress (made; the network suite passes on x86; the gate left) · **Blocked by:** — · **Main task:** [109](../issues-done/109-session-parsers.done) · **Constitution:** MC-3.3, MC-11.11, Appendix B.6

## Problem

[109-NET-0010](109-NET-0010-a-parser-per-session.md) needs a parser process for each session, "started by the session's launcher with no client at all, not even the standard ones `loader` gives a program". The maintainer asked the kernel session to do it (2026-10-12).

`loader` could not:

- it refused to load `parse`, a boot service's name;
- it gave every program the five standard clients (`rtc`, `vfs_server`, `audio_gw`, `loader`, `tts`), a screen unless the program asked for none, and in debug mode the log.

The kernel needs no change: a spawn's capabilities are exactly its grant list.

## Plan

- **`loader.wit` 1.8 `helper(which, served)`**, `which` from `enum helper-kind { parse }`: the caller lends an endpoint that carries the right to receive.
- **The helper gets only that endpoint,** moved into its SLOT_INIT (slot 1, where a service serves), and `--session` among its arguments. No standard clients, no screen, no log even in debug mode, no tasks or endpoints of its own, 2 MiB of memory.
- **At most 4 live helpers per caller,** 32 in all (`limit`). An endpoint without the right to receive is refused (`rights`).

## Acceptance criteria

- [x] `loader.wit` 1.8 and its generated bindings; the IDL fuzzer knows `HelperKind`.
- [x] In the network suite, a download's helper holds one capability, its endpoint in slot 1 (`stat caps`).
- [ ] The gate on x86 and aarch64.

## Related

[109-NET-0010](109-NET-0010-a-parser-per-session.md), `loader/src/main.rs`, [docs/profile/bootstrap.md](../docs/profile/bootstrap.md) (rule 5).
