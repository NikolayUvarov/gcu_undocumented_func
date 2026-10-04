# 151 — Shell grant slots 13–15: authority view, keyboard control, screen capture

**Type:** kernel · **Owner:** kernel track (`common/abi.rs`, `init`) · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Roadmap:** track G, T3 · **Constitution:** MC-3.3, MC-3.11

## Problem

Three planned tools need a client the shell does not have, in a fixed slot that only the kernel track may name (`common/abi.rs` reserves 13–15):

- `caps` (081) needs a `sysmon` client with an authority badge, separate from the plain `SLOT_SYSINFO` client the shell lends to monitors;
- `keymap` (085) needs a client of `ps2_kbd`, which has no service endpoint;
- `screenshot` (086) needs a client of `compositor`, which has no service endpoint either.

## Plan

- `common/abi.rs`: `SLOT_AUTHORITY` 13, `SLOT_KEYBOARD` 14, `SLOT_DISPLAY` 15 (shell grants; the comment on 13–15 updated).
- `init`: endpoints for `ps2_kbd` and `compositor` (`SLOT_SERVICE` receive children, keepers kept across restarts like the other services); the shell gets a `sysmon` client badged `mind::stat::BADGE_AUTHORITY` (1) in 13, a `ps2_kbd` client in 14 and a `compositor` client in 15.
- The services keep working while nobody calls them (the tools issues add the request handling); `docs/profile/bootstrap.md` lists the new grants.

## Acceptance criteria

- `stat caps <shell>` shows endpoint capabilities in slots 13–15 with the authority badge on 13; `ps2_kbd` and `compositor` are servers of their endpoints in `endpoints`; all suites pass.

## Related

[081](081-caps-tool.md), [085](085-keymap.md), [086](086-screenshot.md).
