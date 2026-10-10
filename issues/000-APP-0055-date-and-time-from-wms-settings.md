# 000-APP-0055 — The date and the time set from `wm`'s Settings

**Type:** tools · **Owner:** tools track (`APP`) · **Priority:** P1 · **Status:** in progress (done in QEMU; the maintainer's run on the MacBook Pro left) · **Blocked by:** — · **Main task:** — · **Roadmap:** track G · **Constitution:** MC-3.11

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

## Progress (2026-10-10)

- **The page** (`wm/src/settings.rs`):
  - it shows the clock each second and has six fields and a `Set the clock` button;
  - ← → step a field, digits replace it, Enter sets;
  - a time the clock does not take is named on the page;
  - without the shell's commands, the page says so.
- **Setting it.**
  - `shell.wit` 1.2 `set-clock(date, seconds)` is a word call, so `wm` can stop waiting (0.3 s) while the shell asks. A buffer call's client must not give up while the server holds its buffer.
  - The shell runs it as `date set …` from a client: in its window, after the user agrees. This is one question, the shell's: `wm` asks nothing itself.
  - Settings closes so the keys reach the shell's window, which `wm` brings to the front (`window` first).
- **Checks:**
  - host tests: the day count and the months' lengths; the page's fields, digits, refusal of 2027-02-30, the button's action, and the case without the shell;
  - the `wm` suite sets the year one on and back, agreed in the shell's window, and reads it on the page.
- **Fixes found by the `wm` suite:**
  - `console`'s match on the shell's errors lacked `invalid`;
  - the page showed the background's clock, which reads the RTC once a minute and the date only after midnight. The page now reads the RTC each second. For two minutes after a set, the background reads it each second too.
- **Checked:**
  - the `wm` suite sets the year one on and back, and the shell's `date` and the page show it;
  - Enter on each of the six fields asks the shell, and no keeps the clock (000-APP-0056);
  - the local gate's host tests and every x86 QEMU group passed (2026-10-10).
- **Left:** the maintainer sets the clock from `wm` on the MacBook Pro.

## Related

[requests-APP.md](requests-APP.md) (where it was recorded).
