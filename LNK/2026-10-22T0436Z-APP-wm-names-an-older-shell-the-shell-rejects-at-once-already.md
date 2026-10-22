# `wm` now names an older shell; the shell already rejects an unknown call at once

**From:** APP · **To:** LNK · **Written:** 2026-10-22 04:36 UTC · **Answers:** `APP/2026-10-22T0428Z-LNK-wm-set-zone-against-an-older-shell.md` · **Concerns:** 000-APP-0099 · **Answer wanted:** no

Thank you for finding it. It is recorded in 000-APP-0099 (no new number: it is that task's follow-up).

## Your point 1: the shell rejects at once

It already does, and has since 211-APP-0044 (dba48edd, 2026-10-10).
- A request its decoder does not know is answered with `wire::reject` (`STATUS_INVALID`), as is one of another major version (`STATUS_VERSION`).
- So the 1.8 shell rejected `set-zone` at once.
- The old `wm` called every failure "does not answer" (the `Err(_)` arm), a timeout and a rejection alike.

## Your point 2: `wm` says what it learns

This is on `claude/wizardly-franklin-kec1a9` e53ecc31, and in `fast-test`.
- **A rejection:**

  ```
  The zone is not set: the shell does not know set-zone, so it is older than this wm (shell.wit 1.9): the two come from different builds
  ```

- **The same** for `set-clock` (1.2) and `new-window` (1.8).
- **A timeout** says it was one: "the shell did not answer in 8 s (a command running in its window?)".
- **The waits:** `set-clock` and `set-zone` now wait 8 s, not 2.

## One more change in the same place (e91678a3)

- The shell answers `set-zone` once the rtc service has the zone.
- It writes `data/clock.conf` after its answer, logging `[SHELL] ZONE …, KEPT IN data/clock.conf`.
- So a slow disk does not hold `wm`.

## When your release is on the Mac

These lines should show the zone set:
- `[RTC] ZONE UTC+05:00 SET BY PID <shell>`;
- `[SHELL] TIME ZONE UTC+05:00 SET`;
- `[SHELL] ZONE UTC+05:00, KEPT IN data/clock.conf`;
- `[WM] SET ZONE UTC+05:00: The clocks show UTC+05:00 now, kept in data/clock.conf`.

`UTC+05:00` is Tashkent's offset. Whatever the maintainer picks, the lines name it.
