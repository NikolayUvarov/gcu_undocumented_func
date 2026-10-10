# 211-APP-0044 — `console` joined to the shell, and `shell` in `wm`'s menu

**Type:** tools (`shell`, `console`, `wm`, `idl`) · **Owner:** tools track (`APP`) · **Priority:** P1 (the maintainer, 2026-10-10: the shell's window from the right-click menu) · **Status:** in progress (the menu's `shell` done; `console` joined to the shell next) · **Blocked by:** — (211-KRN-0058, `SLOT_SHELL`, reached `main` on 2026-10-10) · **Main task:** [211](211-intel-pc-from-a-sata-ssd.md) · **Roadmap:** track G · **Constitution:** MC-3.11, MC-11.5

Split from [211-APP-0040](../issues-done/211-APP-0040-the-shells-commands-in-console.done) on 2026-10-10, when that task took the shell's own window, which needs no new slot. Numbered with the kernel track's request in `requests-APP.md` ("The camera from `wm`, and the shell in a window", 2026-10-10, the maintainer's request after the run on the MacBook Pro).

## Problem

211-APP-0040 gives the shell a window of its own in `wm`. What it leaves needs a channel from `wm` and `console` to the shell:

- `console` from the right-click menu still names the shell's commands and refuses them. The maintainer asks that it work fully, as the shell does.
- `wm`'s menu cannot open the shell's window: only Ctrl+Alt+F5 does (211-APP-0045: the shell no longer opens it as it starts `wm`). The maintainer wants it from the menu.
- A program that needs what `wm` does not hold (the network, the log, the lifecycle client, the firmware's settings) runs without it when `wm` starts it. The shell could start it on its own authority and let it open its window in `wm`.

A second shell in every window is not the way: it would spread the authority to reboot, kill, change the network policy and the firmware's boot order.

## Plan

- **`idl/shell.wit` 1.0**, served by the shell:
  - `run(line, confirmed)` runs one command line as if typed on the shell's screen, on the shell's own authority. What it prints goes to an output endpoint the caller lends (the `mind::output` chunks a console program's output already uses), and the answer is the command's status.
  - A command the shell does not take from a client is refused with its name. One that changes the machine (`reboot`, `stop`, `kill`) is refused with a question until `confirmed` is set.
  - `start(program, args)`: the shell starts a program on its own authority, lending it its broker client, so the program opens its window in `wm` with what the shell would give it on its screen.
  - `window()`: the shell opens its window again if it is closed.
- **The shell** serves it between keys, where it tends its consoles, and decides which commands it takes from a client: the observing ones (`ps`, `logs`, `stat`, `cpus`, `free`, `faults`, `quotas`, `heap`, `devices`, `irqs`, `endpoints`, `pmap`), the network diagnostics (`ip`, `net`, `nslookup`, `fetch`, `https`, `netgrants`), `sync`, `logger`, and with confirmation `kill`, `stop`, `boot`, `reboot`. Not `fg`, `voice`, `keymap`, `screenshot` or `msh`: they act on the shell's own screen.
- **The endpoint reaches `console`** through the launchers: the shell lends it in `SLOT_SHELL` to a program that asks for it (`REQUEST_SHELL`); `wm` asks for it and passes it on to `console` (and to nothing else).
- **`console`** sends a line it does not know, or one of the shell's commands, to the shell, shows what comes back, and asks in its window before a confirmed command (`reboot? y/n`). Without the endpoint it names the shell's commands as now.
- **`wm`'s menus** gain `shell` (the right-click desktop menu, and the programs' menu Alt+P), which asks the shell for its window (the maintainer, 2026-10-10: from the right-click menu too, besides Ctrl+Alt+F5).
- **Docs:** `docs/tools` (EN, RU), `idl/shell.wit`, the help of the shell and `console`.

## Acceptance criteria

- **The `wm` suite:** `console` from the menu runs `ps`; `reboot` there asks and, once confirmed, resets the machine (QEMU exits under `-no-reboot`); `fg 1` is refused with a message; the menu's `shell` opens the shell's window after it was closed.
- **Host tests:** the shell's choice of commands from a client (taken, refused, confirmation asked); `console`'s routing of a line (its own command, the shell's, a program).

## Progress (2026-10-10)

- **The menu's `shell` (done).**
  - `idl/shell.wit` 1.0 has the one call the menu needs: `window` opens the shell's window, or answers the open one, and gives its id in the broker.
  - The shell makes the endpoint at the first lend, lends a client in `SLOT_SHELL` to a program that asks for `REQUEST_SHELL` (`mind::process`, bit 20), and serves it between keys.
  - `wm` asks for it. Where it holds it, its menus (the right click on the desktop, Alt+P) start with `Shell (its window)`. `wm` calls `window`, waits at most 2 s, and brings the window to the front once it shows.
  - `mind::idl::wire::with_timeout` gives a word call a timeout. A shell busy with a command (a question it waits on in its own window, whose keys come through `wm`) cannot hold `wm`.
  - Checks: the `wm` suite opens the window from the menu after Ctrl+Alt+F5's was closed, and brings it to the front from behind another window (the `tablet` suite's menu clicks move a row down). Host tests: the menu's first item and its command (`tests/wm_host.rs`), and the timeout of a call (`tests/idl_host.rs`).
- **Next:** `run(line, confirmed)` and `start` in `shell.wit` 1.1; `console` sending the shell's commands; `wm` passing the client to `console`.

## Related

[211-APP-0040](../issues-done/211-APP-0040-the-shells-commands-in-console.done), [158-APP-0043](158-APP-0043-the-camera-from-wm-and-console.md), [u004](../issues-done/u004-console.done), [u006](../issues-done/u006-console-commands.done), [000-APP-0032](000-APP-0032-system-clipboard.md) (the other slot asked for).
