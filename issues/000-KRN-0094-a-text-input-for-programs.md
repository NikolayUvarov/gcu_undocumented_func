# 000-KRN-0094 — A text input for programs, for the shell's pipes and `<`

**Type:** kernel ABI (system calls, ABI 9), `loader.wit` 1.9, `libmind` · **Owner:** kernel session · **Priority:** P2 · **Status:** in progress (in `main` at 289e84b; the tools track's `|` and `<` left) · **Blocked by:** — (the tools track's redirection and pipes wait for it: [requests-APP.md](requests-APP.md)) · **Main task:** — · **Constitution:** MC-12.4, MC-2.6, MC-3.3

## Problem

The maintainer (2026-10-12): "redirection and pipes have to be added to the shell". `dmesg -s battery > file.txt` does not work: the shell has no `>`, `<` or `|`.

**What exists:**

- **A program's output** goes into its console queue in the kernel (`SYSCALL_LOG`). The shell reads it with `SYSCALL_CONSOLE_READ` on the control capability, and that read lets the writer go on when the queue was full (000-KRN-0030). So the shell can already send a program's output to a file or to another program, and nothing is lost.
- **A program's input** is keys only (`SYSCALL_INPUT_EVENT`, `mind::input::read_key`). There is no stream of text a program can read to its end.

So `>` and `>>` need nothing from the kernel. `<` and `|` need a text input with an end.

## Plan

The first plan let the holder of the control capability write any task's input. As made, the right to write one task's input is a capability of its own, given only to the program's launcher (MC-3.3): no other task can write it, and the shell needs no new privilege.

1. **`SPAWN_TEXT`:** a task spawned with it gets a text input, a queue of `TEXT_QUEUE` (4096) bytes in the kernel, charged with its kernel structures. Its writer, a `CAP_KIND_TEXT` capability, goes to the spawner, its handle in `arg2` of the reply; `CAP_INFO` names the reader's PID.
2. **`TEXT_WRITE (writer, bytes, close)`:** queues what fits and answers how many, 0 while the queue is full, so the writer waits instead of losing bytes. `TEXT_CLOSE` with every byte queued ends the input after them. `ERR_PEER` once the reader ended, `ERR_RIGHTS` without a writer.
3. **`TEXT_READ (buffer)`,** for the task itself: what is queued; 0 at the end, which comes when the input was closed or no copy of the writer is left, and stays. With nothing queued it waits up to `TEXT_WAIT_MS` (1 s) for bytes, the close or a key, then answers `ERR_BUSY`: the caller asks again, and can look at its keys meanwhile.
4. **A task started without one** has no text input: the read answers `ERR_NOT_FOUND` at once, and keys stay its input.
5. **`loader.wit` 1.9 `commit-with-input (session, front)`:** `commit`, or with `front` `commit-in-front`, with `SPAWN_TEXT`; the writer is moved to the caller in the reply, and dropped if the caller does not take it, so the program's input ends.
6. **`libmind`:** `mind::io` (`has_input`, `read`, `read_ready`, `read_to_end`, `Lines`, and the launcher's `Writer` with `write_some`, `write_all`, `close`); `mind::process::spawn_fed`.
7. **ABI 9:** `common/abi.rs`, docs/api/README.md, docs/idl/README.md (`own<text>`), docs/profile (kernel objects, bootstrap), tests/runtime.rs.

## Acceptance criteria

- [ ] **The `bench` suite's `check`, on x86 and aarch64:**
  - [ ] a copy of `check` reads its input to the end and reports the bytes, their hash and how it ended; its parent writes 100 KiB to it, more than the queue holds, then closes. Every byte arrives, the end is seen, and a read after it answers the end again (`text input: to its end`);
  - [ ] `check` itself, started without an input, is told `ERR_NOT_FOUND` at once (`text input: none`);
  - [ ] a write through an endpoint instead of a writer is refused with `ERR_RIGHTS`, and a write to a reader that ended with `ERR_PEER` (`text input: rights`);
  - [ ] a writer dropped without a close still brings the reader to its end, after the bytes queued (`text input: writer gone`).
- [x] **`Lines`** cuts lines across any reads, with `\r\n`, a last line without its end, and a line longer than `LINE_MAX` in pieces (tests/io_host.rs).
- [ ] **The tools track's `|` and `<`** run on it ([requests-APP.md](requests-APP.md)).

## Progress (2026-10-20)

- The `bench` suite's `check` passed every text-input row (`none`, `to its end`, `rights`, `writer gone`) on x86 (un-rizen-service) and aarch64 (i9) in the gate of 10d3ec3, now in `main` at 289e84b.
- Left: the tools track's `|` and `<` on top of it ([requests-APP.md](requests-APP.md)).

## Related

[requests-APP.md](requests-APP.md) (redirection and pipes), 000-KRN-0030 (the console queue that waits for its reader), [docs/msh.md](../docs/msh.md).
