# 800-LNK-0011 — `effector485`: a small Effector over the link, for local use

**Type:** tool · **Owner:** `LNK` · **Priority:** P1 · **Status:** in progress · **Blocked by:** — · **Main task:** [800](800-mind-link.md) · **Constitution:** MC-2.3, MC-10.2, MC-12.1, MC-12.3

## Problem

The maintainer asked on 2026-10-20: "link has to be like a small Effector … let's create effector485 for local use … with the ability to run, move and change programs, as a pair with a Linux or Windows program". `KRN` recorded the request in `requests-LNK.md`.
- **How agents drive Effector machines:** through operations, each with an id, an idempotency key and a reason, read for their status and output and cancelled by id ([docs/effector.md](../docs/effector.md), section 3).
- **What a MIND Core machine on the cable had:** only `mindlink`'s one-shot commands. An agent could not hand it a batch of work and come back to it.

## Plan

1. **On the host, `effector485`.** It is a second binary of `tools/mindlink`, for Linux and Windows, std only. It has:
   - a daemon that holds the cable as its one master and keeps the queue and the results in a state file, which a restart keeps;
   - an HTTP API on 127.0.0.1 with Effector's calls and fields (`/api/v1/operations`, `…/cancel`, `/api/v1/endpoints`), and a key file when wanted;
   - a command line: `submit … --key … --reason … [--wait]`, `status`, `wait`, `output`, `list`, `cancel`, `endpoints`.
2. **Operations:**
   - hello;
   - files: put, get, list, stat, mkdir, move, delete, rmdir (800-LNK-0010);
   - programs: run, with its output and exit code, a time limit and cancel (800-LNK-0009);
   - install and uninstall under `apps/`;
   - log from a cursor.

   They run one at a time over the cable. Programs are followed between them.
3. **On MIND Core, `link`** carries them out with the grants `init` gives it. Each power beyond files is a word in `link.conf`: `run` for programs. A reboot client and wider scopes are not asked for now.
4. **Reuse of Effector's contract** (750-EFF-0001): the operation's fields, statuses and the cancel's rules are Effector's, so an agent's code serves both. This is not the native Effector agent of track `EFF`: it is a local tool on the maintainer's machine.

## Acceptance criteria

- [x] **A host test drives `effector485` against `mindlink serve --run`** over a pseudo-terminal (`tools/mindlink/tests/effector485.rs`), through:
  - a batch: put, mkdir, move, list, run with its output and exit code, and log;
  - an operation submitted again with the same idempotency key, done once;
  - a cancel that stops a running program;
  - a restarted daemon that keeps its results and keys.
- [ ] **The same batch against MIND Core in QEMU,** through the `link` suite. It needs 800-LNK-0009's endpoint from `KRN`.
- [x] **Against the MacBook Pro over the RS-485 cable**, from Windows (2026-10-21, below).
- [x] **`cancel` stops a program on MIND Core:** on the MacBook Pro, 2026-10-21 (below).

Evidence for the configurations named only (MC-12.1). The tool claims nothing of Effector's guarantees beyond what its tests show (MC-12.3).

## Progress

**2026-10-20, written; the host test passes.**
- **`effector485.exe`** is built and placed beside `mindlink.exe` on the maintainer's Windows machine.
- **2026-10-21, the MacBook Pro** (`fast-test` b09b7bb), `effector485.exe daemon COM22 --baud 460800` on Windows:
  - `endpoints` showed the Mac online;
  - **a `put` into `apps/`, missing there, was refused** (NOT_FOUND, as the protocol says). `put` now makes the parent directories first, and the host test checks it;
  - `install` of `memtest`, then `run` with `alloc 2`: its output in the operation, completed;
  - **the same idempotency key again** answered the same completed operation, and the program did not run again;
  - `log` from 0, and `list` of `apps/`.
- **2026-10-21, a cancel on the MacBook Pro** (`fast-test` d2daa430, with `link`'s stop through the loader, 800-KRN-0138):
  - a `run` of `t/mt.elf hold` was running, with `[MEMTEST] HELD 16 MiB INTACT=true` in its output;
  - `effector485 cancel` took it to `cancelled`, with `stopped_by: cancel` and no exit code; it had run 14 s;
  - the Mac's log, read by a `log` operation: `RUN 1554972676: STOP ASKED BY THE MASTER`, then `STOPPED … (PID 40, EXIT -1, 34 BYTES OF OUTPUT)`.

## Related

[800-LNK-0009](800-LNK-0009-programs-started-for-the-master.md), [800-LNK-0010](800-LNK-0010-directories-in-data-link.md), [800-KRN-0132](../issues-done/800-KRN-0132-link-starts-programs-for-the-master.done), [750-EFF-0001](750-EFF-0001-protocol-contract.md), [docs/link/effector485.md](../docs/link/effector485.md), [docs/effector.md](../docs/effector.md).
