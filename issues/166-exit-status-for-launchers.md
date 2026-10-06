# 166 — An exit status a launcher can read

**Type:** kernel (process lifecycle) · **Owner:** kernel track · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Roadmap:** track G · **Constitution:** MC-3.3, RFC 001 Marain §7.5 (result model)

Opened from [094](../issues-done/094-shell-script-language.done) (msh), whose plan left this kernel part for when the interpreter needs it.

## Problem

A program ends without saying whether it succeeded. `msh` gives every command a result (`ok` or `err(reason)`). For the shell's own commands it reads their `ERROR:` lines; a program it starts is `ok` once it started, whatever happened then. `grep` finding nothing, `fsck` finding damage, `fetch` failing: a script cannot tell them from success without parsing text.

## Plan

- **`EXIT` with a code.** `mind::process::exit(code)`, the code a `u32` (0: success); returning from `main` is 0, a fault or a kill a code of its own (`EXIT_FAULT`, `EXIT_KILLED`).
- **Readable by the launcher**, through what it already has:
  - the exit notice of `TASK_WATCH` (the post the watcher's endpoint gets) carries the code;
  - for the shell, the focus owner's `NOTICE` of an exited foreground task carries it too, and so does reading an exited console program's last output.
- Nobody else learns it: the code goes where the program's end is already reported (MC-3.3).
- **libmind:** `mind::process::exit`, and `ExitStatus` in what `watch` returns. Programs that fail say so: `grep` (no match: 1), `fsck`, `fetch`, `find`.

## Acceptance criteria

- QEMU `shell` suite: a script with `grep nothing-matches docs/notes.txt or { print("no match") }` prints `no match`; a killed program's code is `EXIT_KILLED`.
- Host test of the notice word's layout.

## Related

[094](../issues-done/094-shell-script-language.done), [162](../issues-done/162-console-output-slot.done), [docs/msh.md](../docs/msh.md).
