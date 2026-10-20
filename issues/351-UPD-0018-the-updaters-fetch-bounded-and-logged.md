# 351-UPD-0018 — The updater's fetch: calls with a deadline, each file in the log

**Type:** update (service) · **Owner:** `UPD` track (worked by the storage session at the maintainer's request) · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [351](351-self-update.md) · **Roadmap:** track C · **Constitution:** MC-5.2, MC-6.1, MC-6.6, MC-12.2

From the kernel track's message to UPD (`internal-dev-messages`, `UPD/2026-10-14T1633Z-KRN-the-updaters-fetch-after-the-cut-stalled.md`).

## Problem

**What was seen.** In the kernel track's gate of b8a7b3d on i9, with another track's gate beside it, the `update` suite stopped at its last fetch.
- The updater refused the changed boot file (`FETCH: REFUSED: Digest`, 75 s).
- The suite then put the blob back and cut the largest one.
- For 1,500 s nothing came on the serial line, from the updater or from a restart.
- The same tree passed the suite alone, and on un-rizen-service.

**What could be at fault** (read from the code):
- **A call that waits for ever.** The updater's fetch has the same shape as `download` had before 351-NET-0029. Its loops give up after 15 s without progress, but only between answered calls. No call to the stack, the TLS service or the parser had a deadline of its own.
- **A slow fetch, logged as nothing.** After the refusal the next fetch takes all 36 boot files again over TLS, under TCG, on a busy machine. The updater logged nothing per file, so a slow fetch and a stalled one looked the same.
- 109-UPD-0017 (8f511a7) is not the cause on its own: the updater holds nothing in `SLOT_LOADER`, so it parses with the shared service, as before.

## Plan

- **A deadline on every call of an attempt.** As in `download` (351-NET-0029), each attempt runs under `wire::with_timeout` at 60 s, above the services' own 10 s.
  - A call that times out counts as a connection that did not answer, and the next try follows.
  - The slot's writes keep no limit.
  - A call that timed out may still take effect (MC-6.6): a session the TLS service opens after the deadline is not used.
- **A resume in the log:** `ASKED FOR THE REST FROM BYTE n`, then `ITS HEAD PARSED: STATUS s`.
- **Each file in the log:** `FETCH: i OF n: <name>, <size> BYTES IN <ms> MS`.
- **The `update` suite prints** how many files the cut fetch logged and the slowest one, as evidence for a slow machine. It is not a check.

## Acceptance criteria

- The `update` suite passes on x86 and aarch64, and shows the resume's lines and each file's time.
- No call of an attempt can wait more than 60 s. This is read from the code: the suite has no service that stops answering.

## Progress (2026-10-20): the times read before the restart

Gate 22's runs printed `TIME: the updater fetched 0 boot files`: the updater's lines go to its log, not to the serial line, and the restart after the fetch loses that log. The update suite now reads `dmesg -s updater` every second while the fetch runs, until the version is staged, and prints each file's time and the resume's line from that attempt. Run locally (x86, QEMU 8.2, TCG, 4 CPUs): 39 boot files, the slowest `tls.elf`, 1379760 bytes in 3354 ms; the resume `ASKED FOR THE REST FROM BYTE 689880`. The suite passed.

## Related

[351-NET-0029](../issues-done/351-NET-0029-download-calls-with-a-deadline.done) (the same for `download`), [109-UPD-0017](109-UPD-0017-the-updaters-checks-in-their-own-parser.md), [351-UPD-0007](../issues-done/351-UPD-0007-updater-service.done).
