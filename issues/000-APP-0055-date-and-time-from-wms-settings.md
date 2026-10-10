# 000-APP-0055 — The date and the time set from `wm`'s Settings

**Type:** tools · **Owner:** tools track (`APP`) · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Main task:** — · **Roadmap:** track G · **Constitution:** MC-3.11

Numbered on 2026-10-10 from `requests-APP.md`, where the kernel track recorded it.

**Recorded by:** the kernel track (KRN), 2026-10-10, at the maintainer's request.

## Problem

The maintainer tried to set the time from `wm` and "it does not react to Enter."

The Settings page "Date and time" (000-APP-0048, `wm/src/settings.rs`) only shows text: "date set YYYY-MM-DD HH:MM[:SS] in the shell's window (Ctrl+Alt+F5) or on its screen". Nothing on it can be chosen or changed, so Enter does nothing. The maintainer's clock is wrong, and `wm` gives no way to correct it.

## Plan

- **The page.**
  - It shows the RTC's date and time, refreshed each second.
  - It has fields for the year, month, day, hours, minutes and seconds. ↑/↓ or typed digits change a field; Tab moves between fields. "Now from the network" can come later, with NTP.
- **Applying it.** Enter (or a "Set" button) asks for confirmation ("Set the clock to 2026-10-10 14:05:00? It keeps no time zone"). It then sets the clock through the shell's command endpoint (`SLOT_SHELL`, `shell.wit`), as `date set …`. The shell alone holds the RTC client with the setting badge (`rtc.wit` 1.2, 211-APP-0042), and `wm` gets no such right of its own.
- **The result** shows on the page and in `wm`'s bar clock: the new time, or why it was refused (an invalid date, the shell's refusal).

## Acceptance criteria

- In the `wm` suite, the page sets the clock, and `date` in the shell then shows the new date.
- An invalid date (2026-02-30) is refused with its reason.
- On the MacBook Pro, the maintainer sets the clock from `wm`.

## Related

[requests-APP.md](requests-APP.md) (where it was recorded).
