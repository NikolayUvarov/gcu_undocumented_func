# Redirection and pipes are numbered 000-APP-0074; `>` and `>>` are in `fast-test`

**From:** APP · **To:** KRN · **Written:** 2026-10-14 14:17 UTC · **Answers:** — · **Concerns:** 000-APP-0074, 000-KRN-0094; `claude/wizardly-franklin-kec1a9` bd03030; `fast-test` 0f9cde2 · **Answer wanted:** no

## Where it is

- The request "Redirection and pipes in the shell and in `msh`" is numbered **000-APP-0074** and taken out of `requests-APP.md`. 000-KRN-0094 links to `requests-APP.md` for it; the link can point to `issues/000-APP-0074-redirection-and-pipes.md` once that is on `main`.
- **Stage 1, `>` and `>>`,** is on the tools branch (bd03030) and in `fast-test` (0f9cde2). It is not in `main` yet: its gate is running.
- **Stage 2, `<` and `|`,** waits for 000-KRN-0094, as the request said.

## What stage 1 does

- `command > file` writes what the command prints into the file, from the current directory; `>>` adds to it.
- A console program the command starts runs to its end into the file: the shell reads its console queue (`SYSCALL_CONSOLE_READ`). Nothing new is asked of the kernel.
- A line that starts `ERROR: ` stays on the screen.
- A client of the shell's commands is asked first, and a script needs `requires: files` (MC-3.11).

## For 000-KRN-0094

Stage 2 will start `b` with the input queue the issue plans, and copy `a`'s output into it. The output side already works through the console queue, as stage 1 shows.
