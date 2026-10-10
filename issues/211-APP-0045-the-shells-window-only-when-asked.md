# 211-APP-0045 — The shell's window opens only when the user asks

**Type:** tools (`shell`) · **Owner:** tools track (`APP`) · **Priority:** P1 (the maintainer's report from the MacBook Pro, 2026-10-10) · **Status:** open · **Blocked by:** — · **Main task:** [211](211-intel-pc-from-a-sata-ssd.md) · **Roadmap:** track G · **Constitution:** MC-11.5

Numbered from the kernel track's request in `requests-APP.md` ("The shell's window opens only when the user asks", 2026-10-10), at the maintainer's report from the MacBook Pro (`fast-test` a00618b): "a program started by itself when `wm` started (the shell). It should not start by itself, only at the user's request from `wm`'s menu."

## Problem

[211-APP-0040](../issues-done/211-APP-0040-the-shells-commands-in-console.done) opens the shell's window whenever the shell starts a window manager, so it appears at every start of `wm`, unasked. The local gate found the same: the `windows` suite's test manager `winmgr` got the shell's window as a third window.

## Plan

- The shell opens its window only when asked: Ctrl+Alt+F5 (the shell takes the key whatever program has the keyboard). Starting a window manager opens nothing.
- `wm`'s menu item `shell` asks the shell for its window through `SLOT_SHELL`: that is [211-APP-0044](211-APP-0044-console-joined-to-the-shell.md), waiting for 211-KRN-0058. Until then the menu has no such item.
- The shell's help and `docs/tools` (EN, RU) say so.

## Acceptance criteria

- **The `wm` suite:** `wm` starts with its three programs' windows and no shell window; Ctrl+Alt+F5 opens it; closed with Alt+W, it goes and `wm` keeps running; Ctrl+Alt+F5 opens it again.
- The `windows` suite passes again (`winmgr` finds the two test windows alone).
- On the MacBook Pro, `wm` starts with the desktop and its programs alone.

## Related

[211-APP-0040](../issues-done/211-APP-0040-the-shells-commands-in-console.done), [211-APP-0044](211-APP-0044-console-joined-to-the-shell.md).
