# 213-APP-0158 — The second screen's fit at the console: `screen second keep|stretch`, kept

**Type:** shell command · **Owner:** tools track (`APP`) · **Priority:** P1 (the maintainer's ask, with the Ryzen on the bench until about 2026-11-23) · **Status:** in progress (written and host-tested, 2026-11-22; the shell suite's run, then the Ryzen) · **Blocked by:** — (`display.wit` 1.4, [213-KRN-0273](213-KRN-0273-the-second-screen-stretched.md), in `main` since round 171) · **Main task:** [213](213-amd-ryzen-pc.md) · **Roadmap:** track G · **Constitution:** MC-12.1

**Recorded by:** the tools track, 2026-11-22, from KRN's request in `issues/requests-APP.md` ("The second screen's fit chosen at the console, and kept").

- **The maintainer** saw the Ryzen's second monitor mirror the first and asked: «Хотелось бы иметь поддержку режимов растягивания, 1 к 1 (как сейчас) и расширения рабочего стола на оба монитора с относительным позиционированием».
- **KRN's `display.wit` 1.4** gives the first two of those: `mirror-fit(keep | stretch)` and `mirror()`.
- **The extended desktop** is [177-APP-0094](177-APP-0094-wm-over-several-monitors.md)'s, on KRN's 177-KRN-0274.

## Problem

The compositor can show the first screen on the second in its own shape or stretched. Nothing lets a person choose, and the compositor forgets the choice when it restarts.

## Plan

- **`screen second`** shows the second screen as `mirror` gives it: its size and the fit, and for `keep` the picture's size. With no monitor offered it says `NO SECOND SCREEN`.
- **`screen second keep|stretch`** (either case) sets the fit with `mirror-fit`, through the shell's own client in `SLOT_DISPLAY`, which has no badge. It is set for the second screen now and for those offered after it. Then it says so and shows the second screen.
- **Kept** in `data/displays.txt`, the file 177's layout is planned in, as a line `second keep|stretch` after a comment. A line the shell does not know is left for that layout.
- **At the shell's start** the file's fit is given to the compositor again, tried as the other kept settings are, and logged: `[SHELL] THE SECOND SCREEN'S FIT FROM data/displays.txt: STRETCH`.
- **A compositor older than 1.4** is named.

## Acceptance criteria

- **Host tests** (`tests/shell_host.rs`): the arguments in either case, the file read back and its last line counting, the words with a screen, without one, and after a choice kept or not.
- **The shell suite in QEMU** (one screen):
  - `screen second` says there is none;
  - `stretch` is taken and kept in the file;
  - a shell killed and started again by `init` sets it from the file;
  - `keep` puts it back.
- **On the Ryzen** (the maintainer's): `screen second stretch` fills the second monitor, and `keep` puts back the first's shape.
- **Docs:** the shell's `help` line, `docs/tools/README.md` and `README_RU.md`.

## Progress (2026-11-22)

- **Written:** `shell/src/screens.rs`, the words and the file, and `screenctl.rs`, the calls and the start; the `help` line; the docs.
- **Tested:** `tests/shell_host.rs` (44 tests, `the_second_screens_fit_shown_set_and_kept` among them). The shell suite's run is under way.

## Related

[213-KRN-0273](213-KRN-0273-the-second-screen-stretched.md), [177-APP-0094](177-APP-0094-wm-over-several-monitors.md) (the extended desktop), [000-APP-0111](000-APP-0111-a-setting-for-the-screens-size.md) (the first screen's size, `screen` too).
