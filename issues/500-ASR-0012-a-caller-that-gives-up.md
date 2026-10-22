# 500-ASR-0012 — A caller that gives up: the server answering into a buffer that is gone survives

**Type:** test (fault injection) · **Owner:** `ASR` · **Priority:** P1 · **Status:** in progress (2026-10-22) · **Blocked by:** `KRN`: [000-KRN-0152](000-KRN-0152-a-revoked-lease-does-not-fault-its-borrower.md) (ABI 17), a revoked lease leaves its borrower a zeroed page of its own · **Main task:** [500](500-fuzzing-abi-and-idl.md) · **Constitution:** MC-2.3, MC-6.1, MC-12.1

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

## Progress

- **2026-10-22, the reproduction.**
  - **`tests/isolation_app.rs` case `G`:**
    - a child lends a page with a call, gives up after 200 ms and revokes the lend, as `wire::call_buffer` does;
    - the parent maps the page as `wire` maps a client's buffer, writes it 400 ms later as a late `reply_buffer` does, has its reply refused (`ERR_PEER`), and serves the child's next message.
  - **The QEMU suite `giveup`:**
    - `qemu_smoke.py --suites giveup --isolation-elf …`, only when named; it is in no CI group;
    - it runs case `G` three times and fails on a fault of the server.
  - **Today it fails as the Mac did:** `FAULT PID=1 CPU=0 VECTOR=14 ERROR=0x6`, the shell's fault of the same afternoon (x86, QEMU, 4 CPUs).
  - **The `isolation` suite still passes** with the new case in the program.
- **2026-10-22, KRN's rule (000-KRN-0152, ABI 17).** On revocation the server's mapping becomes a zeroed page of its own, so its late write goes nowhere.
  - As KRN suggested, case `G` now also checks the caller's side. The child keeps its page past the server's late write, then reads it: nothing of that write may be there (MC-2.6).
  - The suite fails on that too.
  - It joins a CI group (x86 and aarch64) once 000-KRN-0152 lands.

## Related

The report on `internal-dev-messages` (APP/, 2026-10-22), 000-APP-0096 (`net-counters`), [500](500-fuzzing-abi-and-idl.md).
