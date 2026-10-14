# 000-APP-0074 — Redirection and pipes in the shell and in `msh`

**Type:** tools (`shell`, `msh`, tools) · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open: `>` and `>>` done; `<` and `|` wait · **Blocked by:** [000-KRN-0094](000-KRN-0094-a-text-input-for-programs.md) for `<` and `|` · **Main task:** — · **Roadmap:** track G · **Constitution:** MC-3.11, MC-11.5

**Recorded by:** the kernel track (KRN), 2026-10-12, in `requests-APP.md`, at the maintainer's request: "redirection and pipes have to be added to the shell". Numbered by the tools track on 2026-10-14; the maintainer chose it as the track's next task.

## Problem

`dmesg -s battery > file.txt` fails, as does `dmesg | grep BATTERY`: the shell has no `>`, `>>`, `<` or `|`.

## Plan

**Stage 1, `>` and `>>`.** This needs nothing from the kernel.

- `shell/src/redirect.rs`:
  - splits a line at its first `>` outside double quotes. One file follows, at the end of the line, and a command started with `&` is refused;
  - sorts the output line by line: a line that starts `ERROR: ` goes to the screen, the rest into the file.
- `shell/src/main.rs` (`redirected`):
  - opens the file first, through the shell's file client, from the current directory. `>` creates or empties it, `>>` adds to its end;
  - runs the command with its output captured;
  - drives a console program it starts to its end (Esc or Ctrl+C stops it), and writes as the output comes;
  - shows how many bytes went in.
  - The shell's `STARTED` line stays on the screen. A program with a screen is named: what it shows is not in the file.
- **Authority** (MC-3.11):
  - a client of the shell's commands is asked in the shell's window for any line with `>`, even when its command alone would be taken at once;
  - a refused command stays refused;
  - a script needs `requires: files` for `>`, as for `write`.
- The help line, `docs/tools` and `docs/msh.md` (EN, RU).

**Stage 2, `<` and `|`.** This needs [000-KRN-0094](000-KRN-0094-a-text-input-for-programs.md).

- The shell starts `b` with an input queue and copies `a`'s output, or the file, into it. It waits when the queue is full and closes it when `a` ends.
- Esc stops the whole line, and the line's result is the last command's.
- The tools read their input when no file is named: `grep`, `sha256`, `cat`, `find`, `view`.
- `msh`: `<` and `|` in a command's words. A command's output as a value is `capture("…")` already.

## Acceptance criteria

- **Stage 1:**
  - **Host tests** (`tests/shell_host.rs`):
    - a line splits at `>` and `>>`, and a quoted `>` is text;
    - a wrong line is refused: no command, no file, two files, two `>`, or `&`;
    - the output's lines go to the file or the screen, whole even when they come in pieces;
    - a client's line with `>` is asked.
  - **The `shell` suite:**
    - `ls data > ram:l.txt`, and `cat ram:l.txt` shows the listing; `>>` adds a second one;
    - `dmesg -s init > d.txt` from `ram:` holds init's lines;
    - `cat` of a missing file into a file shows the error and writes 0 bytes;
    - a file in a missing directory and two words after `>` are refused;
    - a script without `requires: files` fails at `>`, and one with it writes the file.
- **Stage 2** (the original request's criteria):
  - `dmesg | grep SHELL` prints only matching lines;
  - `cat log:boot0001.log | sha256` equals `sha256 log:boot0001.log`;
  - 100 KiB through `|` arrive whole.
- **On the MacBook Pro:** `dmesg -s battery > data/battery.txt` leaves the file on the stick.

## Related

[000-KRN-0094](000-KRN-0094-a-text-input-for-programs.md) (the kernel's text input), [094](../issues-done/094-shell-script-language.done) (`msh`), [000-APP-0066](../issues-done/000-APP-0066-cd-pwd-and-tab-completing-paths.done) (the current directory), [211-APP-0044](../issues-done/211-APP-0044-console-joined-to-the-shell.done) (the shell's clients).
