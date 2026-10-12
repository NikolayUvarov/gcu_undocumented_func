# 000-APP-0066 — `cd` and `pwd`, and Tab completing paths, in the shell and `console`

**Type:** tools · **Owner:** tools track (`APP`) · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Main task:** — · **Roadmap:** track G · **Constitution:** MC-11.5

**Recorded by:** the kernel track (KRN), 2026-10-12, from the maintainer's run of `fast-test` c548a85 on the MacBook Pro, where `diag/quick.msh` ([176-KRN-0087](../issues-done/176-KRN-0087-runs-on-real-hardware.done)) was run by its path ([requests-APP.md](requests-APP.md), taken from there). Enter on a script in `fm` is [000-APP-0067](000-APP-0067-fm-runs-a-script.md).

## Problem

The maintainer:

- "`cd diag` does not work: there is no such command."
- "Tab does not complete a path: `diag/q` is not completed to the name, or part of it."
- "No list of the programs that match what was typed is shown."
- "I cannot run the script … in its directory by typing its name."

In the shell (`shell/src/main.rs`, `complete`):

- **There is no current directory**, so every path is written from the volume's root.
- **Tab completes only the first word**, or the word after `run`, and only with command and program names. A word holding `/` or `:` matches nothing.
- **The list** is printed only when the matches share nothing beyond what was typed.
- **`console` has no Tab at all.**

## Plan

- **`mind::path`** (`libmind/src/path.rs`), shared by the shell and `console`:
  - a path taken from the current directory: a volume or a leading `/` starts at a root, `.` and `..` work;
  - `pwd`'s form (`/`, `/diag`, `ram:notes`) and the prompt's (`MIND diag> `);
  - Tab's completion of a name from a list.
- **The shell:**
  - `cd [directory]` and `pwd`. Each console and the shell's window has its own directory. `cd` alone goes to the boot disk's root, `cd ..` one level up, and `cd` to a missing directory says so.
  - A path without a volume starts at the current directory in `ls`, `cat`, `write`, `mkdir`, `rm`, `mv`, `msh`, a script by its name (`quick.msh`), `release`, `screenshot`, and the scripts' `files()` and `glob()`.
  - A program has no directory of its own, so the shell passes it full paths: a word of its arguments that names a file or directory in the current directory, and the first word of a program that asks for its file (`edit new.txt`).
  - A script's `cd` lasts while the script runs.
- **Tab:**
  - The first word, and the one after `run`, are completed from the commands and programs.
  - A word holding `/` or `:`, or any later word, is completed from the directory it names.
  - One match gets its full name, with `/` after a directory. Several get the part they share; once they share nothing more, Tab lists them, sorted.
- **`console`:** `cd`, `pwd`, the prompt `diag> `, the same paths for its own commands and the programs it starts, and the same Tab.
- **`help`** (the shell and `console`) names `cd`, `pwd` and Tab; `docs/tools` (EN, RU).

## Acceptance criteria

- **Host tests:**
  - `tests/path_host.rs`: paths from a directory, `..` at a root, the volumes; `pwd`'s and the prompt's forms; Tab's completion, its shared part, its list, letter case, Cyrillic.
  - `tests/console_host.rs`: `cd`, `pwd` and the prompt.
- **The shell suite** (x86 and aarch64):
  - `cd diag`, and the prompt is `MIND diag> `; `pwd` says `/diag`;
  - `msh --check quick.msh` works there, and `ls` lists it;
  - `cd nowhere` says the directory is not found; `cd ..` goes back;
  - Tab after `cat diag/qu` gives `cat diag/quick.msh`;
  - Tab after `c` lists the commands and programs.
- **The `wm` suite:** in `console`, `cd diag`, `ls`, Tab after `cat qu`, `cd` and `pwd`.

## Related

[176-KRN-0087](../issues-done/176-KRN-0087-runs-on-real-hardware.done) (the diagnostic scripts), [000-APP-0067](000-APP-0067-fm-runs-a-script.md), [094](../issues-done/094-shell-script-language.done) (`msh`), [u006](../issues-done/u006-console-commands.done) (`console`'s commands).
