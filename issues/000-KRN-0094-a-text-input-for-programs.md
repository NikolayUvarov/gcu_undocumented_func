# 000-KRN-0094 — A text input for programs, for the shell's pipes and `<`

**Type:** kernel ABI (system calls, ABI 9), `libmind` · **Owner:** kernel session · **Priority:** P2 · **Status:** open · **Blocked by:** — (the tools track's redirection and pipes wait for it: [requests-APP.md](requests-APP.md)) · **Main task:** — · **Constitution:** MC-12.4, MC-2.6

## Problem

The maintainer (2026-10-12): "redirection and pipes have to be added to the shell". `dmesg -s battery > file.txt` does not work: the shell has no `>`, `<` or `|`.

**What exists:**

- **A program's output** goes into its console queue in the kernel (`SYSCALL_LOG`). The shell reads it with `SYSCALL_CONSOLE_READ` on the control capability, and that read lets the writer go on when the queue was full (000-KRN-0030). So the shell can already send a program's output to a file or to another program, and nothing is lost.
- **A program's input** is keys only (`SYSCALL_INPUT_EVENT`, `mind::input::read_key`). There is no stream of text a program can read to its end.

So `>` and `>>` need nothing from the kernel. `<` and `|` need a text input with an end.

## Plan

1. **Each task gets an input queue** in the kernel, beside its console queue, bounded the same way (4 KiB a call).
2. **`SYSCALL_CONSOLE_WRITE (pid, bytes)`**, for the holder of the control capability (the shell): appends to the task's input queue. It answers how many bytes fit, as `SYSCALL_LOG` does, so the shell waits instead of losing bytes. `SYSCALL_CONSOLE_WRITE (pid, close)` marks the end.
3. **`SYSCALL_INPUT_READ (buffer)`**, for the task itself: what is queued; 0 at the end. It waits while the queue is empty and not closed, and a key or the close wakes it.
4. **A task started without a pipe** has no input queue: the read answers `ERR_NOT_FOUND`, and keys stay its input.
5. **`libmind` `mind::io`:** `stdin()`, an iterator of lines (`read_line`) and `read_to_end` into a buffer, for the tools that take input when no file is named.
6. **ABI 9:** the two system calls; docs/api/README.md; tests/runtime.rs.

## Acceptance criteria

- **A QEMU suite** (`isolation` or `shell`): a test program reads its input to the end and prints what it counted; the shell writes 100 KiB to it, more than the queue holds, then closes. Every byte arrives, and the end is seen once.
- **A program that does not hold the control capability** cannot write another's input.
- **A read with no queue** answers `ERR_NOT_FOUND` at once, and keys still reach the program.
- **The tools track's `|` and `<`** run on it ([requests-APP.md](requests-APP.md)).

## Related

[requests-APP.md](requests-APP.md) (redirection and pipes), 000-KRN-0030 (the console queue that waits for its reader), [docs/msh.md](../docs/msh.md).
