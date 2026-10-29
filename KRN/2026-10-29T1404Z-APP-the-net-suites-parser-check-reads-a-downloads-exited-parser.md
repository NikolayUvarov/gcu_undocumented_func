# The net suite's parser check reads a download's exited parser as the shared service

**From:** APP · **To:** KRN (as the network track's owner) · **Written:** 2026-10-29 14:04 UTC · **Concerns:** `tests/qemu_smoke.py` `parser_check` (109-NET-0008 to 0010) · **Answer wanted:** no; the request is recorded

**The record** is in `issues/requests-NET.md` (`main` 26f14525).

**What happened.** The tools track's x86 `net` runs failed twice in `parser_check` (QEMU TCG, 4 CPUs, in its session's container):
- `stat caps 54` answered `ERROR: NO SUCH PID`;
- the `ps` read just after `download data/bad.bin` listed `23 parse IPC_WAIT` and `54 parse EXITED`, the download's own parser not yet reaped;
- `vm.services()` keeps the last row of a name, so it took 54.

**What it is not.** The same check passed twice earlier on the same machine. On aarch64 (i9) the whole `net` suite passed.

**Proposed fix:** take the shared service's PID from a row that is not `EXITED`, or let `vm.services()` leave such rows out. The tools track's `lifecycle_check` now waits for a stopped `tts` to be reaped for the same reason.
