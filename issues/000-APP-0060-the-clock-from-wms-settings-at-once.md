# 000-APP-0060 — The clock set from `wm`'s Settings at once, without the shell's question

**Type:** tools · **Owner:** tools track (`APP`) · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Main task:** — · **Roadmap:** track G · **Constitution:** MC-3.11, MC-11.5

**Recorded by:** the maintainer, 2026-10-11: "the clock setting in wm's menu does work in the end, but it must not open a new shell window asking whether to set the time. It must set the time itself, at once, with no extra question."

## Problem

Since 000-APP-0055, Enter on the date page of `wm`'s Settings calls `set-clock` (`shell.wit` 1.2). The shell treats it as `date set` from any client of its commands:

- Settings close;
- the shell opens its own window in front and asks `RUN IT? (Y/N)`;
- only after `y` there is the clock set.

The user already chose the time in Settings, so the question repeats a decision just made.

## Plan

- **The shell.** It records the PID of each window manager it starts with its commands (PIDs are never reused). `set-clock` from such a PID sets the clock at once through the shell's RTC client, with no question and no window. Any other client of the shell's commands (a program `wm` passed them on to, such as `console`) is still asked in the shell's window, so the clock does not pass to every program that asks for the shell's commands.
- **`shell.wit` 1.3.** The interface states it.
- **`wm`.** It no longer brings the shell's window up. Settings stay open, and the page shows the clock and the outcome (`The clock is set to …`, or why not).
- **Docs.** `docs/tools` (EN, RU), `docs/idl`, the profile's line on the shell's clients.

## Acceptance criteria

- In QEMU, the `wm` suite sets the year one on and back from Settings: the shell logs that it ran `date set` at once, the page shows the new year, Settings stay open, and no window opens.
- Enter on each of the date page's six fields sets the clock at once.
- A client of the shell's commands that is not its window manager is still asked (unchanged path; the `wm` suite's `console` checks `kill` that way).

## Related

[000-APP-0055](000-APP-0055-date-and-time-from-wms-settings.md) (the date page and `set-clock`), [211-APP-0044](../issues-done/211-APP-0044-console-joined-to-the-shell.done) (the shell's commands).
