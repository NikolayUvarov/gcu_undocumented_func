# 000-APP-0097 — Several windows of the shell, each a session of its own

**Type:** tools (`shell`, `wm`, interface) · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** — (follows [211-APP-0040](../issues-done/211-APP-0040-the-shells-commands-in-console.done)) · **Roadmap:** track D · **Constitution:** MC-3.11, MC-12.3, MC-12.4

**Recorded by:** the tools track, 2026-10-21, from the maintainer's report: "when i try to run second shell i got into first open, may be we should fix it", and the question that followed: why only 4 windows, and what if more are needed.

## Problem

- **The shell keeps one window.** It has four consoles and a fifth session for its window in `wm` (211-APP-0040).
- **A second `Shell` from `wm`'s menu brings back the first one.** `shell.wit` `window` answers the window that is open, and Ctrl+Alt+F5 does the same. There is no way to have two shells side by side in `wm`.

## Plan

- **The shell keeps up to 32 window sessions** after its four consoles, each with its own text, line, history, scrollback, `msh` statements and current directory.
  - The windows are titled `shell`, `shell 2`, `shell 3` and so on. `ps` names a program started in one `CONSOLE=5` and on.
  - Each window takes about 1 MiB of the shell's memory: a ring of 400 lines at the screen's width, 4 bytes a cell, and its buffers.
  - The window broker keeps 64 windows for every program, so 32 is past what the machine can hold beside other programs' windows. The limit only bounds the shell's table.
- **`idl/shell.wit` 1.8 `new-window`:** another window beside those open, answering its id. `busy` with 32 open; `no-memory` or `unavailable` when the shell's memory or the broker has no room.
  - `window` (1.0) stays as it was: the first window open, or a new one when none is. A client's command that asks first (`console`'s `run`) is asked there.
  - The authority stays the shell's, as for the first window (MC-3.11).
- **`wm`'s `Shell` menu item asks for `new-window`** and says why when the shell cannot open one.
- **Ctrl+Alt+F5 opens another window** each time. The shell still listens for Ctrl+Alt+F1…F5 alone.

## Acceptance criteria

- **The `wm` suite** (x86, aarch64):
  - the first `Shell` from the menu opens `shell`, and `cd diag` there changes its prompt;
  - a second `Shell` opens `shell 2`, a window of its own whose banner says `THE SHELL'S WINDOW 2:` and whose prompt is not the first one's;
  - both close.
- **Host tests:** `shell_host` and `idl_fuzz_host` pass with `shell.wit` 1.8.

## Related

[211-APP-0040](../issues-done/211-APP-0040-the-shells-commands-in-console.done) (the shell's window), [211-APP-0044](../issues-done/211-APP-0044-console-joined-to-the-shell.done) (`shell.wit`, the menu item), [155](../issues-done/155-virtual-consoles.done) (the consoles), [211-APP-0045](../issues-done/211-APP-0045-the-shells-window-only-when-asked.done) (Ctrl+Alt+F5).
