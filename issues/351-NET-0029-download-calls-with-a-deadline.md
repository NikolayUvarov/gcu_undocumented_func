# 351-NET-0029 — `download`'s calls with a deadline, and its resume in the log

**Type:** network (program) · **Owner:** `NET` track (worked by the storage session at the maintainer's request) · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [351](351-self-update.md) · **Roadmap:** track D · **Constitution:** MC-5.2, MC-6.1, MC-6.6, MC-12.2

From the request "A download hung once at its resume on x86 with 6 GiB, after 109-NET-0010's fix" in requests-NET.md (recorded by the kernel track, 2026-10-14, from its gate of 63ef648 on i9).

## Problem

**What was seen.** In the `net` suite's `download_check`, `download` printed `CONNECTION CUT AT 10485760 OF 31457280, RESUMING` and then nothing for 900 s. The same check passed elsewhere in the same gate and on a second run.

**Whether a resume could wait for ever, and where.** It could. The loops in `download` gave up after 15 s without progress, but only between calls that were answered. No call of an attempt had a deadline of its own:
- to the network stack through the flow grant (`tcp_connect`, `tcp_send`, `tcp_receive`, `close`);
- to the TLS service (`attach`, `connect`, `send`, `receive`, `close`);
- to its parser (`http-response`) and to the loader for a new parser.

The kernel fails a call to a parser that has ended (ERR_PEER), but not one to a service that is alive and does not answer. The log cannot say which call it was: nothing was logged between the cut and the head.

## Plan

- Every call of an attempt runs under `wire::with_timeout` at 60 s, above the services' own 10 s. A call that times out counts as a connection that did not answer, and the next try follows. A parser that does not answer ends the run with `HTTP: Parser` after `Session` tried a new one. The file's writes keep no limit.
  - A call that timed out may still take effect (MC-6.6): a connection the stack opens after the deadline is not used, and the next try opens its own.
- A resume logs its request (`DOWNLOAD: ASKED FOR THE REST FROM BYTE n`) and its head (`DOWNLOAD: ITS HEAD PARSED: STATUS s`).
- The `net` suite requires both lines after the cut, over HTTP and HTTPS. docs/network/downloads.md 0.4 says so.

## Acceptance criteria

- The `net` suite passes on x86 and aarch64 with the two new lines after each cut.
- No call `download` makes during an attempt can wait more than 60 s. This is read from the code: the suite has no service that stops answering.

What this does not show: why the call on i9 got no answer. If it happens again, the log names the step and the run goes on.

## Progress (2026-10-14)

- **The same hang again, before this change:** the tools track's gate of 70764d5 (which changes only `wm`) saw "x86: RAM above 4 GiB" stop on i9 after the display suite, until Effector's hour ended it. Its QEMU logs were cleared before they were read, so the step is not known (message to NET on `internal-dev-messages`, 2026-10-14).
- **With this change,** gate 15 (bb81a81) passed the network suite on x86 locally (both new lines required after each cut) and on aarch64 on un-rizen-service, and "x86: RAM above 4 GiB" locally (856 s). One pass is not a proof that the hang is gone; it shows the new lines and bounds do not break the suite.

## Related

[351-NET-0001](../issues-done/351-NET-0001-http-downloads.done), [109-NET-0010](109-NET-0010-a-parser-per-session.md), [351-NET-0002](../issues-done/351-NET-0002-https-for-programs.done).
