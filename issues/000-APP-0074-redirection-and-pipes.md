# 000-APP-0074 — Redirection and pipes in the shell and in `msh`

**Type:** tools (`shell`, `msh`, tools) · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open: `>`, `>>`, `<` and `|` done, several `|` in a line too; the MacBook Pro's check left (the maintainer's) · **Blocked by:** — ([000-KRN-0094](000-KRN-0094-a-text-input-for-programs.md)'s text input is in `main`) · **Main task:** — · **Roadmap:** track G · **Constitution:** MC-3.11, MC-11.5

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

- The shell starts `b` with a text input: `loader.wit` 1.9 `commit-with-input` (or with `front`, as `commit-in-front`) returns the input's end for the shell (the kernel track's plan, on its branch).
- The shell copies `a`'s output, or the file, into it with `write_all`, which waits while the kernel's 4 KiB queue is full, and closes it when `a` ends.
- `b` reads it with `mind::io::read` or `Lines`, to 0 at its end. A program started without an input gets `Error::NotFound` (`has_input`).
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

## Progress (2026-10-22): `|`

- **The maintainer's need:** `ps | grep wm` did not run.
- **The kernel's part** (000-KRN-0094) has been in `main` since 289e84b: a program started with `loader.wit` 1.9 `commit-with-input` reads a text input whose writer its launcher holds.
- **The shell** (`shell/src/pipe.rs`, `piped_line`, `feed_more`):
  - `command | program` gathers what the command prints, as `>` does. That includes a console program before `|`, which runs to its end first, and the `ERROR: ` lines, which stay on the screen.
  - It then starts the program with that as its input.
  - Its main loop fills the input as the program reads, so neither waits on the other, and closes it at the end.
  - One `|` a line. The program after it is one of the disk's; a shell command there is named and refused.
  - `| … > file` works through `>`.
- **`grep` without a path** reads its text input; without one, its usage (`search/src/bin/grep.rs`).
- **Tests:**
  - host tests (`tests/shell_host.rs`: the split; `tests/search_host.rs`: grep without a path);
  - the `find`/`grep` check: `cat | grep`, `ps | grep shell`, `ls | grep -c`, a line that matches nothing, the refusals.
- **Left:** `<`; more than one `|`.

## Progress (2026-10-29): `<` and several `|`

- **Several `|`** (`shell/src/pipe.rs`, `piped_stage`): `dmesg | grep SHELL | grep -c START`.
  - Each program runs to its end in turn, its input filled as `|` fills one, and what it prints is gathered for the next.
  - The whole line is checked before anything runs: an empty side, `||`, `&`, and a shell command after any `|`.
- **`program < file`** (`redirect::input`, `input_line`):
  - the file, from the current directory, is read whole, 1 MiB at most, and becomes the program's text input;
  - `<` belongs to the first command of the line (`grep a < f | grep b`); after a `|` it is refused;
  - its command is a program of the disk's, as after `|`; `> file` may follow.
- **`sha256` without a file** hashes its text input and names it `-`, as `sha256sum` does.
- **`msh`** runs its command lines through the shell, so `<` and `|` work in scripts as at the prompt.
- **Not taken up:** `view` reading its input. `cat` and `find` are not readers: `cat` is the shell's command, and `find` lists names.
- **Tests:**
  - host tests (`tests/shell_host.rs`): the pipe chain and its refusals, `<` and its refusals;
  - the `find`/`grep` check:
    - `cat | grep | grep -c`, `ps | grep shell | grep -c shell`;
    - `grep hello < ram:a.txt`;
    - `sha256 < docs/pipe.txt` and `cat docs/pipe.txt | sha256` against `sha256 docs/pipe.txt` and the host's digest: 156 KiB whole through `<` and through `|`; `grep -c` counts its 2048 lines through `<`;
    - the refusals.

  `docs/pipe.txt`, which the `tools` suite puts on its disk, stands in for `log:boot0001.log` in the stage 2 criterion: a log being written could change between two reads.
- **Left:** the MacBook Pro's `dmesg -s battery > data/battery.txt`, the maintainer's run on the machine.

## Related

[000-KRN-0094](000-KRN-0094-a-text-input-for-programs.md) (the kernel's text input), [094](../issues-done/094-shell-script-language.done) (`msh`), [000-APP-0066](../issues-done/000-APP-0066-cd-pwd-and-tab-completing-paths.done) (the current directory), [211-APP-0044](../issues-done/211-APP-0044-console-joined-to-the-shell.done) (the shell's clients).
