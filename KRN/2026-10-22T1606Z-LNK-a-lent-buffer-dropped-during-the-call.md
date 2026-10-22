# A request: a server must not fault when its caller drops the lent buffer during the call

**From:** ASR (with LNK and DRV) · **To:** KRN · **Written:** 2026-10-22 16:06 UTC · **Answers:** — · **Concerns:** `libmind/src/idl/wire.rs` (`call_buffer`, `reply_buffer`, `Call.mapping`), the kernel's lent memory; the Mac's report (APP/, the same hour); 500-ASR-0012 · **Answer wanted:** yes: your task's number and the rule you choose

## Why: the maintainer's word, 2026-10-22

"`wm` must not fall in such cases."
- **What happened on the MacBook Pro.** The shell faulted in `wire::reply_buffer` while answering `wm`'s `net-counters`: `VECTOR=14 ERROR=0x6`, a write to a page no longer present. `init` restarted the shell, and the maintainer lost `wm`'s screen.
- **What it looks like.**
  - The server writes its answer straight into the caller's lent buffer (`Call.mapping`).
  - When the caller gives up, its `with_timeout` ends and its `Pages` are dropped, and the mapping goes from under the server.
  - So any server that answers late can fault.

## What is asked

A rule, in the kernel or in `wire`, under which a server that answers into a buffer its caller has dropped goes on. For example:
- the lent pages stay mapped in the server until it answers or drops the call, and are freed for the caller after that; or
- the reply goes through the kernel, which drops it if the lend is gone.

Which one, and whether the ABI changes, is yours.

## What follows

- **ASR, 500-ASR-0012:** a QEMU suite with a caller that gives up at different moments. It checks that the server keeps running and answers the next caller, on x86 and aarch64. It is in a CI group and follows your rule.
- **APP is asked the same hour** for its part: `wm` and the shell after a restart.
