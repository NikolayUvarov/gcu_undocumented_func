# 035 — Key events: E0 keys, modifiers, layouts, VT100, event queue

**Type:** bug/feature · **Priority:** P0 · **Status:** open · **Blocked by:** — · **Roadmap:** track G, tools plan F4 · **Constitution:** MC-1.1

## Problem

- `ps2_kbd` drops every E0-prefixed key: arrows, Home/End, PgUp/PgDn, Ins/Del, right Ctrl/Alt never reach a program.
- A focused application gets raw PS/2 make codes or raw UART bytes in one byte stream: 0x1E is both the scan code of "A" and a UART byte; every program decodes scan codes itself (`app2`, `dzen-clock`).
- No modifiers, no key release state, no keyboard layouts (no Russian input from PS/2), no F-keys, no VT100 sequences from the UART.

## Plan

- **One event format** (`common/abi.rs`): a 32-bit word — bits 0–20 Unicode character (0 if none), bits 21–27 key code (`KEY_ENTER`, `KEY_ESC`, `KEY_BACKSPACE`, `KEY_TAB`, arrows, `KEY_HOME`, `KEY_END`, `KEY_PGUP`, `KEY_PGDN`, `KEY_INSERT`, `KEY_DELETE`, `KEY_F1`–`KEY_F12`; 0 for a plain character), bits 28–30 Shift, Ctrl, Alt. Enter, Esc, Tab and Backspace also carry their control character, so text consumers keep working; Ctrl+letter carries the letter with Ctrl.
- **Kernel** (mechanism only): the per-task input queue holds events (64 entries) instead of bytes; `INPUT_EVENT` takes one event word and the attention flag; `READ_KEY` returns the next event word. No decoding or layout in the kernel.
- **`ps2_kbd`** (set 1 through the i8042 translation): E0 keys, break codes, Shift/Ctrl/Alt state, Caps Lock, keypad navigation, F1–F12, Pause skipped; layouts US and Russian ЙЦУКЕН. The layout switches with Ctrl+Shift or Alt+Shift pressed and released without another key (so Ctrl+Shift+arrow still selects). Ctrl+Z stays the attention key.
- **Shell** (owner of COM1): decoder of VT100/xterm input — CSI and SS3 sequences for arrows, Home/End, Ins/Del, PgUp/PgDn, F1–F12 with modifier parameters; 0x7F and 0x08 as Backspace; CR, LF and CRLF as one Enter; control bytes as Ctrl+letter; UTF-8 sequences as one character; a lone Esc after 50 ms.
- **`mind::input`**: `Key` type with `char()`, `code()`, modifiers; `read_key() -> Option<Key>`, `wait_key(ms)`, `wait_or_exit`. Port `app`, `app2`, `dzen-clock`, `listen`, `shell`.
- **Decisions** (tools plan §8, item 5): Norton Commander / FAR key conventions; layout switch Ctrl+Shift or Alt+Shift; Ctrl+Z is reserved for the system, so undo in tools is Ctrl+U or Alt+Backspace.

## Acceptance criteria

- Host tests for the PS/2 and VT100 decoders (`tests/keys_host.rs`).
- QEMU suite `keys`: arrows, Home, Delete, F-keys and Shift/Ctrl from PS/2 (`sendkey`) and from the UART reach an application as the same events; the Russian layout gives Cyrillic; all existing suites pass.

## Related

[docs/tools](../docs/tools/README.md) F4; README "Console".
