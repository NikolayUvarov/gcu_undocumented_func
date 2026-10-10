# 000-APP-0054 — The shell's and `console`'s commands answer the help keys

**Type:** tools · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** — · **Roadmap:** track G · **Constitution:** MC-11.5

Numbered on 2026-10-10 from `requests-APP.md`, where the kernel track recorded it.

**Recorded by:** the kernel track (KRN), 2026-10-10, at the maintainer's request: "`date` prints the date but answers none of `/help`, `--help`, `-help`; all programs must answer the help keys in the same way."

## Problem

- **Programs.** Since 000-KRN-0066, every program built with `libmind` answers `--help`, `-help`, `-h`, `/help`, `/h`, `/?` and `-?` (any case) with its text (`mind::process::HELP_KEYS`, `asks_help`).
- **The shell's commands.** `date`, `time`, `ps`, `ls` and the rest answer none of them. `date --help` gets "THIS COMMAND TAKES NO ARGUMENTS", and `ls --help` looks for a file. The same holds for the commands of `wm`'s `console`.
- **Programs with a screen.** The shell's `help_instead` (`shell/src/main.rs`) shows a screen program's text from its file only for `--help`. With `fm -h`, the program now prints its text and exits, out of sight.

## Plan

- **In the shell and in `console`.** Before a command parses its arguments, `asks_help(args)` turns `<command> <help key>` into `help <command>`: the command's lines from `HELP`.
- **`help_instead`.** It uses `asks_help` in place of `args != b"--help"`.
- **`docs/tools` (EN, RU).** One line on the help keys.

## Acceptance criteria

- In the shell and in `console`, `date -h`, `date /?`, `ls --help` and `ps -help` print their lines from `help`.
- `fm /?` shows fm's text without starting it.
- The shell suite checks a few.

## Related

[requests-APP.md](requests-APP.md) (where it was recorded).
