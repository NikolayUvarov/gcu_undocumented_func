# 000-APP-0065 — `poweroff` and `shutdown`, and "Restart" and "Turn off" in `wm`'s menu

**Type:** tools · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** — · **Roadmap:** track G · **Constitution:** MC-11.5

**Recorded by:** the kernel track (KRN), 2026-10-11, at the maintainer's request: "there is no shutdown, and it is needed while we have to turn the system off". The maintainer asked again on 2026-10-12, once `reboot --off` had turned the MacBook Pro off: "a synonym `poweroff` is needed for this command" ([requests-APP.md](requests-APP.md), taken from there).

## Problem

`reboot --off` writes the caches, stops the services and turns the machine off. Since [000-KRN-0080](../issues-done/000-KRN-0080-power-off-on-x86.done) it does so on x86 too (ACPI S5), not only on aarch64 (PSCI). But:

- nobody looks for it under `reboot`;
- `wm`'s menu has no entry to restart or to turn off. Its System category names a program `reboot`, which does not exist;
- `shell/src/power.rs` still says "aarch64 only", and its refusal does not say why.

## Plan

- **In the shell.**
  - `poweroff [-f]` and `shutdown [-f]` do what `reboot [-f] --off` does.
  - Typed in the shell's window, they ask `TURN THE MACHINE OFF? (Y/N)`, as `reboot` asks `RESTART THE MACHINE?`.
  - `help` names them on `reboot`'s line, so `help poweroff` and `help shutdown` find it.
  - A script needs the word `lifecycle` for them, as for `reboot`.
  - Where the firmware names no S5, the refusal says so and that `reboot` restarts the machine.
- **From a client** (`console`, `idl/shell.wit` `run`): both are asked in the shell's window, as `reboot` is. `console` passes them to the shell.
- **In `wm`'s menu.**
  - System ends with `Restart` (`console reboot`) and `Turn off` (`console poweroff`) where `wm` holds the shell's commands and `console` is there to pass them on.
  - The shell asks in its window. Its question is the confirmation.
  - The program `reboot` leaves the category list.
- **`docs/tools`** (EN, RU): the names, the menu entries, ACPI S5 and PSCI.

## Acceptance criteria

- **Host tests.**
  - `tests/wm_host.rs`: System ends with `Restart` and `Turn off`, whose commands are `console reboot` and `console poweroff`; without `console` they are not there.
  - `tests/shell_host.rs`: `poweroff` and `shutdown -f` are asked of a client.
  - `tests/console_host.rs`: `console` passes both to the shell, which asks.
- **The shell suite** (x86 and aarch64): `help poweroff` and `help shutdown` show the line; `poweroff now` gets the usage; `poweroff -f` turns the machine off (`POWER OFF VIA ACPI S5`, `PSCI SYSTEM_OFF`) and QEMU ends.
- **The `wm` suite** (x86):
  - typed in the shell's window, `poweroff` asks, and `n` keeps the machine running;
  - the menu walk finds System's `Restart` and `Turn off`; Enter on each opens `console`, the shell asks its question in its window, and `n` keeps the machine running.

## Related

- [084](../issues-done/084-reboot.done): `reboot`.
- [000-KRN-0080](../issues-done/000-KRN-0080-power-off-on-x86.done): power off on x86. [203](../issues-done/203-aarch64-smp-and-power.done): PSCI.
- 211-APP-0044: the shell's commands from `wm` and `console`. 000-APP-0056: every entry of the menu reacts.
