# 750-APP-0105 — The shell's `mode` command: the machine's modes on and off

**Type:** tools (`shell`, `msh`) · **Owner:** tools track (`APP`) · **Priority:** P1 (the maintainer's table of 2026-10-22: the first task that eases tests on real hardware) · **Status:** open · **Blocked by:** — (`init.wit` 1.5 and `SLOT_MODE` are in `main`, 750-KRN-0143) · **Main task:** [750](750-effector-agent.md) · **Roadmap:** the development of MIND Core on real hardware · **Constitution:** MC-3.8, MC-8.1, MC-10.2, MC-12.3, MC-12.6

**Recorded by:** the tools track, 2026-10-22, from 750-EFF-0013's item 3 and KRN's notice (`ALL/2026-10-21T1658Z-KRN-modes-in-fast-test-init-wit-1-5-abi-14.md`: "APP: when the `mode` command is planned"). It should have been numbered then; the maintainer's table of 2026-10-22 named it again.

## Problem

- **What runs the Effector agent.** `init` runs the agent's services only in the development or the maintenance mode, kept in `data/mode.conf` (750-KRN-0143).
- **What turns a mode on.** At run time only `init.wit` 1.5 `set-mode`, through the client badged `BADGE_MODE` that the shell alone holds in `SLOT_MODE`.
- **The shell had no command for it.** The owner had to write `data/mode.conf` at the keyboard and restart.

## Plan (750-EFF-0013, item 3)

- `mode`: which modes are on, who set them last and why (`mode()`).
- `mode development|maintenance on|off [reason]`: `set-mode` with the shell's `BADGE_MODE`.
  - **On:** asks the operator at the local console first, saying what the agent may then do.
  - **Off:** no question.
  - The reason goes to `init`'s log line, `operator` without one.
- **Not over the link, not for a lent client.** `clients::taken_by` refuses `mode` to every client the shell lends and to the link's.
- **A script at this console** may run it, and is asked the same.
- **Visible** (item 5): while a mode is on, the shell says so above its first prompt and in the system log.
- **It replaces** 750-APP-0093's `effector enable|disable`.

## Acceptance criteria

- **Host tests** (`tests/shell_host.rs`):
  - the command line and its limits;
  - the state line, the question and the notice;
  - every lent client refused.
- **The QEMU `modes` suite,** on a disk whose `data/mode.conf` says `development`:
  - the notice above the prompt;
  - `mode` shows the development mode set by `disk`;
  - `mode maintenance on` answered N changes nothing; answered Y, `init` logs the change with the shell's badge;
  - `mode development off` turns that mode off, and `data/mode.conf` keeps `maintenance`.
- **Docs:** `docs/tools/README.md` and `README_RU.md`, and the help line.

The test runs show the command on the configurations they ran on (MC-12.1). What the agent may do in a mode is the gateway's and the owner's policy's (750-EFF-0013, item 6), not this command's.

## Related

- [750-EFF-0013](750-EFF-0013-development-and-maintenance-modes.md), [750-KRN-0143](750-KRN-0143-init-follows-the-development-and-maintenance-modes.md).
- [750-APP-0093](750-APP-0093-the-shells-effector-command.md): its `enable` and `disable` give way to this.
