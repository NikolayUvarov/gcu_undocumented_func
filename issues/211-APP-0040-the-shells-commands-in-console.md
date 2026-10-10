# 211-APP-0040 — The shell's commands in `wm`: the shell's own window

**Type:** tools (`shell`, `console`, `wm`) · **Owner:** tools track (`APP`) · **Priority:** P1 (raised by the maintainer, 2026-10-10: without a proper console `wm` is hard to work in) · **Status:** in progress · **Blocked by:** — · **Main task:** [211](211-intel-pc-from-a-sata-ssd.md) · **Roadmap:** track G · **Constitution:** MC-3.11, MC-11.5

Numbered from the kernel track's request in `requests-APP.md` (2026-10-09, at the maintainer's request after a run on the MacBook Pro). Its second request ("The camera from `wm`, and the shell in a window", 2026-10-10) asks for the same: the shell in a window, and the difference between `console` and the shell explained. What needs a channel from `wm` and `console` to the shell went to [211-APP-0044](211-APP-0044-console-joined-to-the-shell.md).

## Problem

Before `wm` starts, every shell command works on the shell's screen. In `wm` the user has `console`, which starts programs and has a few commands of its own (`ps`, `ls`, `cat`, `date`, `ping`, …), but it names the shell's other commands and refuses them: `reboot`, `kill`, `logs`, `stop`, `boot`, `sync`, `ip`, `nslookup`, `fetch`, `stat`, `cpus` and the rest need what only the shell holds (process control, the lifecycle client, the operator's network client). A second shell in every window would spread those authorities.

## Plan

The request offered two ways: `console` sends lines to the shell over a lent endpoint, or a window that is a view of the shell's own session. The first needs a new fixed slot from `KRN` (asked for on 2026-10-09; every application slot below `SLOT_DYNAMIC` is named). The second needs none, and it is taken (2026-10-10, at the maintainer's raised priority):

- **The shell's window.** When the shell starts a window manager, it opens a text window of its own through its broker client (`SLOT_WINDOWS`), titled `shell`, and shows there a session of its own, a fifth console beside the four of Ctrl+Alt+F1…F4: its line editing, history, Tab, scrollback, `msh` statements and every command, run by the shell on its own authority as on its screen. Nothing is lent to anyone, and `wm` only shows the window and passes it the keys, as it does for every window.
- **Programs started there** open as windows in `wm` (the shell lends them its broker client, as `wm` does), not over `wm` on a screen of their own; a console program prints into the shell's window, as on the shell's screen.
- **The window follows its frame:** the session's lines and rows take the size `wm` gives it.
- **Closed** with `[×]` or Alt+W, the window goes; Ctrl+Alt+F5 (the shell listens for it, as for F1…F4) or the next window manager the shell starts opens it again.
- **`reboot` and `stop` ask** in the window before they act: its keys come through the window manager.
- **`fg` is refused** there: the window has no screen to give a program; a window manager is started from a console.
- `libmind` gains what this needs: a window opened through a broker client of the program's choosing (`mind::windowed::Window`), and a `mind::tui::Terminal` over such a window.
- `console` keeps its own commands; its help points to the shell's window for the shell's commands.
- **Docs:** `docs/tools` (EN, RU) and the shell's help, with the difference between the shell and `console` and its reason: the shell is the one holder of the operator's authorities; `console` is a terminal for programs with what `wm` holds; a second full shell in every window would spread the authority to reboot, kill, change the network policy and the firmware's boot order.

## Acceptance criteria

- **The `wm` suite:** `wm` started from the shell shows the shell's window; typed there, `ps` lists the tasks, `logs` of a program and `date` answer, a program started there opens as a window, and the window resized by `wm` shows its prompt again at its new size; `reboot` there asks, and `n` keeps the machine running; closed, the window goes, and Ctrl+Alt+F5 opens it again.
- **Host tests:** the console's resizing (`tests/console_host.rs` or the shell's own).

## Related

[211-APP-0044](211-APP-0044-console-joined-to-the-shell.md), [158-APP-0043](158-APP-0043-the-camera-from-wm-and-console.md), [u004](../issues-done/u004-console.done), [u006](../issues-done/u006-console-commands.done), [088](../issues-done/088-text-window-manager.done).
