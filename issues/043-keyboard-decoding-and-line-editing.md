# 043 — Keyboard decoding, layouts, VT100 input and shell line editing (tools F4, ring 3)

**Type:** tool · **Owner:** tools track · **Priority:** P1 · **Status:** open · **Roadmap:** track G, T0 · **Blocked by:** — (034 done)

## Problem

`ps2_kbd` drops E0-prefixed keys and has no releases, modifiers or layouts; the shell passes raw UART bytes and has no line editing or history.

## Plan

- `ps2_kbd`: E0 prefix, break codes, modifiers, Caps Lock, US and Russian layouts with a switch key, events via issue 034.
- Shell: VT100/xterm decoder with a lone-Esc timeout; line editing and history.
- `mind::input::read_event`, `wait_event`.

## Acceptance criteria

- Host tests of the PS/2 set-1 and VT100 decoders; QEMU suite `keys`: arrows and F-keys from UART and PS/2 reach an application.

## Related

[docs/tools/README.md](../docs/tools/README.md) (plan of the tools track, branch `claude/wizardly-franklin-kec1a9`) F4.
