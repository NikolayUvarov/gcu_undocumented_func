# 500-ASR-0012 — A caller that gives up: the server answering into a buffer that is gone survives

**Type:** test (fault injection) · **Owner:** `ASR` · **Priority:** P1 · **Status:** open (2026-10-22) · **Blocked by:** `KRN`: what the kernel does with a lent buffer its caller drops during the call (asked 2026-10-22) · **Main task:** [500](500-fuzzing-abi-and-idl.md) · **Constitution:** MC-2.3, MC-6.1, MC-12.1

## Problem

On 2026-10-22 the MacBook Pro's shell faulted while answering `wm`'s `net-counters`:
- `FAULT PID=33 VECTOR=14 ERROR=0x6`, in `mind::idl::wire::reply_buffer`;
- a write to a page that was no longer there. The likely cause is a reply buffer its caller had dropped after its call timed out, while the card was changing.
- `init` restarted the shell, and the maintainer lost `wm`'s screen.

The fixes are other tracks':
- KRN's: the lent buffer and `wire`;
- APP's: the shell and `wm`.

Whatever they choose, a server must outlive a caller that gives up. No test checks that today.

## Plan

A test program that calls a service with a lent buffer and gives up before the answer: its `with_timeout` ends, or it drops the buffer, or it exits.
- **The service under test** answers late on purpose: a stand-in, or a real one made slow.
- **A QEMU suite** runs it many times, with different moments of giving up.
- **It checks that:**
  - the service is still running;
  - it answers the next caller;
  - nothing faults in the log.

The kernel's chosen behaviour, once KRN states it, is checked as stated.

## Acceptance criteria

- [ ] The suite passes on x86 and aarch64 once KRN's and APP's fixes are in; it fails on the code of 2026-10-22, the case seen on the Mac.
- [ ] It is in a CI group.

Evidence for the configurations named only (MC-12.1).

## Related

The report on `internal-dev-messages` (APP/, 2026-10-22), 000-APP-0096 (`net-counters`), [500](500-fuzzing-abi-and-idl.md).
