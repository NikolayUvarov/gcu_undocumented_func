# 800-LNK-0009 — Programs started for the master: RUN, their output and their end

**Type:** protocol, service, tool · **Owner:** `LNK` · **Priority:** P1 · **Status:** in progress · **Blocked by:** `KRN`: an endpoint quota of 1 for `link` (LNK's request of 2026-10-20 on `internal-dev-messages`); for STOP and the exit code, the two grants asked of `KRN` the same day · **Main task:** [800](800-mind-link.md) · **Constitution:** MC-2.3, MC-3.11, MC-10.2, MC-11.4, MC-12.1, MC-12.4

## Problem

The maintainer asked on 2026-10-20 to test programs on MIND Core without a reboot and without a person at the machine. The request was recorded by `KRN` in `requests-LNK.md` ("Starting a program for the master"), with [800-KRN-0132](../issues-done/800-KRN-0132-link-starts-programs-for-the-master.done).
- **What already worked:** the agent could put a program into `data/link/` (`mindlink put`).
- **What was missing:** only someone at the keyboard could start it (`run data/link/<name>.elf`).

## Plan

1. **The protocol: a programs channel (5)**, in [docs/link/protocol.md](../docs/link/protocol.md), section 5:
   - RUN: a name and arguments; it answers the run's number and the PID;
   - OUTPUT: from a cursor, with the run's state and exit code;
   - STOP;
   - RUNS.

   A slave that predates it refuses these with UNSUPPORTED (MC-12.4).
2. **`link`:**
   - It starts `data/link/<name>` through a launch session of the loader client `init` lends it (800-KRN-0132), so the program gets an application's grants.
   - It lends the program a send copy of an endpoint of its own in `SLOT_CONSOLE`. It keeps the last 16 KiB of each run's output, and sees the end with `mind::process::alive`.
   - It starts nothing unless `link.conf` holds the word `run`.
   - It logs each start and end (MC-10.2).
3. **`mindlink`:**
   - `run`, `start`, `out`, `runs` and `stop`.
   - `serve --run`, which starts programs in its directory as the serving user; it ends them with STOP and knows their exit codes.
4. **Not in this task:** ending a program on MIND Core, and its exit code. Both need grants from `KRN`, asked on 2026-10-20. Until then STOP is refused with UNSUPPORTED, and the exit code is unknown.

## Acceptance criteria

- [x] The protocol's requests through a line that loses, repeats and echoes frames: a repeated RUN starts one program; output read past what the slave dropped; a stop (`linkproto/tests/link.rs`).
- [x] `mindlink` against `mindlink serve --run`: output, exit code and a stop; nothing started without `--run` (`tools/mindlink/tests/mindlink.rs`).
- [ ] The `link` suite puts a test program into `data/link/`, starts it with `mindlink run`, reads its output and its end, and a name the loader cannot take is refused. This needs `KRN`'s endpoint quota for `link`. It passed in a local run with that one line applied and not committed; the committed suite checks the refusal and prints a SKIP until it lands.
- [ ] The MacBook Pro over the RS-485 cable: a program put and run from the development machine, its output read.

Evidence for the configurations named only (MC-12.1).

## Progress

**2026-10-20, written; the host tests pass; QEMU with a local `init` line.**
- **`linkproto`:** the programs channel; its host test is `programs_over_a_lossy_line`.
- **`link`:** `Programs` in `link/src/main.rs`. Output is taken between frames, within 20 ms, while a program runs.
- **`mindlink`:** the commands above, and `serve --run`. A program put over the link gets its owner's execute bit when it is run.
- **Local QEMU runs:**
  - **With `init`'s line `"link" => Quota { endpoints: 1, ..Quota::default() }` applied in the working tree only,** the `link` suite:
    - started `memtest` from `data/link/t/mt.elf` twice, with and without `--help`, and read its output to its end;
    - listed the runs;
    - had a missing program and a path over the loader's 64 bytes refused;
    - got STOP refused with UNSUPPORTED.
  - **Without that line:**
    - `link` logs `[LINK] NO ENDPOINT FOR PROGRAMS' OUTPUT (…)` (ERR_LIMIT);
    - RUN is refused with UNSUPPORTED;
    - the suite says SKIP for the program part.
- **Not done:** the Mac. Its image predates this task.

## Related

[800-KRN-0132](../issues-done/800-KRN-0132-link-starts-programs-for-the-master.done), [800-LNK-0003](800-LNK-0003-the-link-service-on-mind-core.md), [800-LNK-0010](800-LNK-0010-directories-in-data-link.md), [800-LNK-0011](800-LNK-0011-effector485.md), [docs/link/protocol.md](../docs/link/protocol.md), [docs/link/mindlink.md](../docs/link/mindlink.md).
