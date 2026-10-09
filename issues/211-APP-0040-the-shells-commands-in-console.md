# 211-APP-0040 — The shell's commands in `wm`'s `console`

**Type:** tools (`shell`, `console`, `wm`) · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open · **Blocked by:** a fixed slot for the shell's command endpoint ([requests-KRN.md](requests-KRN.md)) · **Main task:** [211](211-intel-pc-from-a-sata-ssd.md) · **Roadmap:** track G · **Constitution:** MC-3.11, MC-11.5

Numbered from the kernel track's request in `requests-APP.md` (2026-10-09, at the maintainer's request after a run on the MacBook Pro).

## Problem

Before `wm` starts, every shell command works on the shell's screen. In `wm` the user has `console`, which starts programs and has a few commands of its own (`ps`, `ls`, `cat`, `date`, `ping`, …), but it names the shell's other commands and refuses them: `reboot`, `kill`, `logs`, `stop`, `boot`, `sync`, `ip`, `nslookup`, `fetch`, `stat`, `cpus` and the rest need what only the shell holds (process control, the lifecycle client, the operator's network client). A second shell in every window would spread those authorities.

## Plan

- **`idl/shell.wit` 1.0**, served by the shell: `run(line, confirmed)` runs one command line as if typed on the shell's screen, on the shell's own authority, and sends what it prints to an output endpoint the caller lends (the `mind::output` chunks a console program's output already uses), then answers with the command's status. A command the shell does not take from a client is refused with its name; one that changes the machine (`reboot`, `halt`, `kill`, `stop`) is refused with a question until `confirmed` is set.
- **The shell** serves it while a program is in front (it polls its endpoint where it polls its notices), and decides which commands it takes: the observing ones (`ps`, `logs`, `stat`, `cpus`, `free`, `faults`, `quotas`, `heap`, `devices`, `irqs`, `endpoints`, `pmap`), the network diagnostics (`ip`, `net`, `nslookup`, `fetch`, `https`, `netgrants`), `sync`, `logger`, and with confirmation `kill`, `stop`, `boot`, `reboot`. Not `fg`, `voice`, `keymap`, `screenshot`, `msh` or the console switch: they act on the shell's own screen.
- **The endpoint reaches `console`** through the launchers: the shell lends it to a program that asks for it (`REQUEST_SHELL`) in the new fixed slot; `wm` asks for it and passes it on to `console` (and to nothing else); `console` started from the shell gets it directly.
- **`console`** sends a line it does not know, or one of the shell's commands, to the shell, shows what comes back, and asks in its window before a confirmed command (`reboot? y/n`). Its help lists the shell's commands it can send. Without the endpoint it names the shell's commands as now.
- **Docs:** `docs/tools` (EN, RU), the shell's and `console`'s help, `idl/shell.wit`.

## Acceptance criteria

- **Host tests:** the shell's choice of commands from a client (taken, refused, confirmation asked); `console`'s routing of a line (its own command, the shell's, a program).
- **The `wm` suite** opens `console`; `ps` there lists the tasks; `reboot` there asks, and once confirmed resets the machine (QEMU exits under `-no-reboot`); `fg 1` is refused with a message.

## Related

[u004](../issues-done/u004-console.done), [u006](../issues-done/u006-console-commands.done), [088](../issues-done/088-text-window-manager.done), [000-APP-0032](000-APP-0032-system-clipboard.md) (the other slot asked for).
