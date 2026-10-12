# 000-APP-0067 — `fm` runs a script: Enter on a `.msh` file

**Type:** tools · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** — · **Roadmap:** track G · **Constitution:** MC-3.11, MC-11.5

**Recorded by:** the tools track, 2026-10-12, split from the maintainer's report in [000-APP-0066](000-APP-0066-cd-pwd-and-tab-completing-paths.md): "I cannot run the script from fm by Enter".

## Problem

- **Enter on `diag/quick.msh` in `fm` shows its text**, as on any file that is not a program (`fm/src/panel.rs`, `is_program`: `.elf` only).
- **Typed on `fm`'s command line**, the name goes to the loader, which answers `Cannot start full.msh: NotFound`.
- **Only the shell runs scripts:** `msh` is the shell's interpreter, holding the shell's authority within the words a script declares.
  - `fm` holds no shell's commands. The shell lends them only to a window manager and to what its own window starts.
  - The shell refuses scripts from a client of its commands (`shell/src/clients.rs`).

## Plan (to decide)

- **In `wm`.** `fm` asks for the shell's commands (`REQUEST_SHELL`). Enter on a script sends `msh <path>` to the shell. The shell takes it from a client as a command it asks about first: `RUN IT?` in its window, where the script then runs and prints. A script that declares words asks for them as it does today for a script outside the boot disk.
- **On the shell's screen,** where `fm` holds no shell's commands, Enter says how to run it: `msh <path>` in the shell.
- **`help`, `fm`'s key bar and `docs/tools`** (EN, RU) name it.

This widens what a client of the shell's commands may ask for: running a script on the shell's authority, after the user agrees in the shell's window. The maintainer decides whether that is wanted.

## Acceptance criteria

- **The `wm` suite:** Enter on `diag/quick.msh` in `fm` asks in the shell's window; yes runs it there, and its first line shows; no leaves it.
- **On the shell's screen,** Enter says to type `msh diag/quick.msh` in the shell.

## Related

[000-APP-0066](000-APP-0066-cd-pwd-and-tab-completing-paths.md), 211-APP-0044 (the shell's commands for clients), [094](../issues-done/094-shell-script-language.done) (`msh`).
