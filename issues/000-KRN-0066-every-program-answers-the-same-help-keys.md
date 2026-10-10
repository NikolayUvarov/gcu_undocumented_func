# 000-KRN-0066 — Every program answers the same help keys

**Type:** kernel (the core of `libmind`) · **Owner:** kernel session · **Priority:** P2 · **Status:** in progress (made and tested in QEMU; the gate left) · **Blocked by:** — · **Roadmap:** track G · **Constitution:** MC-12.3

## Problem

The maintainer (2026-10-10): `date` answers none of `/help`, `--help` and `-help`, and "all programs must answer the help keys in the same way."

For programs, `mind::about!` printed the program's text only for `--help` exactly. Any other key was passed on as an ordinary argument. `-h`, `/?` and `-help` therefore started the program, or were refused as a wrong argument, depending on the program.

`date` itself is a command of the shell, not a program, so the shell's commands are the tools track's part (requested in `requests-APP.md`).

## Plan

- `mind::process::HELP_KEYS`: `--help`, `-help`, `-h`, `/help`, `/h`, `/?` and `-?`, in any case.
- `mind::process::asks_help(args)`: whether the whole argument text is one of them.
- `about!` uses it, so every program built with `libmind` answers them alike.
- The shell's own check for a program with a screen (`help_instead`, which shows the text from the file instead of starting the program) is the tools track's to switch to `asks_help`. That is requested too.

## Acceptance criteria

1. A console program prints its text for each key. The services suite checks `uptime` with all seven keys and `--HELP`.
2. No program took any of these keys for something else. Checked: no program's arguments use them.

## Progress

**2026-10-10.** Done in `libmind/src/process.rs`; the services suite passes on x86 with the new checks.

## Related

`requests-APP.md`: the shell's commands answer the same keys, and `help_instead` uses `asks_help`.
