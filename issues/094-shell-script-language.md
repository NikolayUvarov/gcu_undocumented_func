# 094 — `msh`: the shell's script language

**Type:** tools (shell, interpreter) · **Owner:** tools track · **Priority:** P2 · **Status:** open · **Blocked by:** — (exit statuses and output capture: a small kernel part, see below) · **Roadmap:** track G, next to track E (Marain) · **Constitution:** MC-3.11 (a request grants nothing), MC-3.3, RFC 001 Marain §7.5 (result model)

## Problem

The shell runs one command at a time from the keyboard or the UART. Nothing can be automated: a start-up sequence, a test, a repeated task, a recipe for the user. A POSIX shell would bring along its quoting rules, global environment and ambient authority. MIND Core needs its own small language that fits its model: results instead of exit codes and strings, capabilities instead of ambient rights.

## Plan

- **Language `msh`**, interpreted by the shell. Scripts are files (`*.msh`), run as `msh file [args]` or as a program name. The same language works at the prompt, so a script is what the user would type.
  - **Commands** as today: `name args…`, `run name &`, the shell's own commands.
  - **Values:** strings, integers, booleans, lists, records (`{name: "x", size: 3}`).
  - **Variables:** `let x = …`, local to a block. Interpolation: `"size: {x.size}"`. No word splitting, no globbing unless asked (`glob("ram:*.txt")`).
  - **Control:** `if`/`else`, `while`, `for item in list`, `fn name(args) { … }`, `return`.
  - **Results, as in Marain** (RFC 001 §7.5):
    - every command and function returns `ok(value)` or `err(reason)`;
    - `cmd?` propagates an error, `cmd or { … }` handles it, `try { … } catch e { … }` handles a block;
    - nothing continues silently after an error, unlike `set -e`, which a shell has to remember.
  - **Output:** `let out = capture(cmd)` takes a command's console output as text; `lines(out)`, `words(text)`, `contains`, `match(pattern)` with `mind::pattern`.
  - **Built-ins** for the system's typed interfaces, so scripts do not parse text where an IDL answer exists: `ps()`, `services()`, `files(dir)`, `ip()`, `log(text)`, `sleep(ms)`, `now()`.
- **Authority:**
  - A script runs with the authority of the shell session that starts it, but no more than it declares.
  - The first line after `#!msh` is `requires: files network log …`, the same words as `mind::request!`. The shell shows them and asks once before a script from outside `A:` uses them.
  - Programs a script starts get what they ask for through the usual launch session, never more than the script declared.
  - A script cannot reach the shell's process control except through declared `requires: lifecycle`.
- **Kernel and loader part, small:**
  - an exit status: `EXIT` with a code, readable by the program's launcher through the existing exit notice (`TASK_WATCH`);
  - capture of a program's console output through the loader session (the shell already reads `LOGS`).
  
  This is done with the kernel owner as a separate kernel issue when the interpreter needs it.
- **Later, with Marain:** `msh` stays the interactive and glue language. A script can call a Marain component as a command. Its syntax for values, records and results follows Marain's, so moving logic from a script into a typed component is a rewrite of the same ideas, not a translation between two worlds.
- **Implementation:**
  - `mind::script`: lexer, parser to an AST, tree-walking interpreter with step and memory budgets (no endless loop can hang the shell: Ctrl+Z or the budget stops a script);
  - the shell uses it for its prompt and for files;
  - `msh -c "…"` for one line; `msh --check file` parses and checks names without running.

## Acceptance criteria

- **Host tests:** the lexer and parser (errors with line and column), evaluation of expressions, results and `?`/`or`/`try`, step-budget stop.
- **QEMU `shell` suite:**
  - a script in `data/` that loops over files on `ram:`, writes a summary with `write`, and handles a missing file with `or`;
  - a script that captures `ps` output and checks a service is running;
  - a script with `requires: network` is refused the network without it, and gets it when declared;
  - Ctrl+Z stops a script in an endless loop.
- **Documentation:** `docs/msh.md` (EN/RU) with the grammar and examples; `help msh`.

## Related

[RFC 001 Marain](../constitution/EN/RFC_001_Marain_v0.4.md), [155](155-virtual-consoles.md), [088](088-text-window-manager.md), [docs/tools](../docs/tools/README.md).
