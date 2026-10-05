# 155 — Virtual consoles: several shell consoles and switching between them

**Type:** shell + kernel · **Owner:** tools track (shell), kernel track (key routing) · **Priority:** P2 · **Status:** open · **Blocked by:** — (the key routing is done: `INPUT_LISTEN`, 154) · **Roadmap:** track G · **Constitution:** MC-3.3, MC-10.2

## Problem

There is one console: the shell's screen and line, with one foreground program at a time. `fg` and Ctrl+Z move the focus between that program and the shell. While a program such as `edit`, `top` or `view` runs in the foreground, there is no second prompt: a second command needs Ctrl+Z, and the program then loses the screen.

## Plan

- **Consoles in the shell.** The shell keeps several consoles (4), each with its own:
  - text buffer and scrollback, input line and history;
  - foreground program, console program and the programs started from it.
  
  The authority stays the same: one user and one shell process. Separate shell processes per console belong to multi-user sessions, later.
- **Switching.** Alt+F1…Alt+F4 work whatever program has the focus. A switch:
  1. redraws the shell's screen with that console's buffer, or
  2. gives the focus to that console's foreground program (`FOCUS`).
  
  A program in a console that is not shown keeps running. Its console output goes to its own console's buffer.
- **Key routing (kernel, done in 154).** The switch keys must reach the shell even while another program has the focus, and never that program. `INPUT_LISTEN` (`mind::input::listen(KEY_F1, MOD_ALT, true)` … `KEY_F1 + 3`) takes each key with exactly those modifiers out of the focused stream and into the shell's own input queue, presses and releases; up to 8 registrations. The shell reads them in its main loop while a program has the focus, as it does for F12. Ctrl+Z keeps its meaning in each console. What remains here is the shell's part.
- **COM1** stays attached to console 1, the test harness's channel. The status line names the console shown. `ps` names the console of every program.
- **Console faces of the clocks** (left from [089](../issues-done/089-text-clock-faces.done)): `clock` and `dzen-clock` started as console programs of a console print the time on one line updated in place. Their text faces for a screen or a `wm` window are done in 089.

## Acceptance criteria

QEMU `keys` and `shell` suites:
- `view` runs in console 1; Alt+F2 shows a new prompt; a command there works; Alt+F1 shows `view` again with the keyboard, and `view` never received Alt+F2.
- Each console keeps its own history and scrollback.
- A program in a console that is not shown keeps running and its output appears when that console is shown again.
- A task without process control cannot register the switch keys (`ERR_RIGHTS`).

## Related

[154](../issues-done/154-push-to-talk-routing.done), [151](../issues-done/151-shell-grant-slots-13-15.done), [docs/tools](../docs/tools/README.md).
