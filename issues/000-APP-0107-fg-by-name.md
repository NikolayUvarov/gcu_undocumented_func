# 000-APP-0107 — `fg` by a program's name, listing them when several answer

**Type:** tools (`shell`) · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** — · **Roadmap:** track D · **Constitution:** MC-12.3

**Recorded by:** the tools track, 2026-10-22, at the maintainer's request: "команда fg должна принимать не только цифровой id, но и строку. Если подходящих программ несколько - должна вывести их pid для выбора."

## Problem

`fg` took a PID alone. The user had to read `ps` to bring `wm` or `clock` to the front.

## Plan

- **`fg <name>`** (`shell/src/pick.rs`): the program called so, in any case; if none is, the one whose name holds the text (`fg dzen`).
- **One** is brought to the front, as `fg <pid>` does.
- **Several:** none is taken. They are listed, `SEVERAL PROGRAMS ANSWER TO clock: PID 1 clock, PID 2 clock, FG <PID> TAKES ONE`.
- **None:** `ERROR: NO PROGRAM CALLED <name> RUNS (PS LISTS THEM)`.
- **Left out:** services and the shell itself.
- `kill` and `logs` keep their PID: a name that picks a program by chance should not end it.

## Acceptance criteria

- **Host tests** (`tests/shell_host.rs`): one, several, none, any case, a part of a name.
- **The QEMU shell suite:**
  - `fg clock` and `fg app` list two each;
  - `fg nosuch` names none;
  - `fg app2` brings PID 5 to the front.
- **Docs:** `docs/tools/README.md`, `README_RU.md` and the help line.

## Related

The same choice by name serves 000-APP-0106: a restarted shell finds `wm`.
