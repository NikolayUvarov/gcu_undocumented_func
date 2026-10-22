# 000-KRN-0152 — A revoked lease does not fault its borrower

**Type:** kernel (`user_heap`, the fault path), ABI (what a revoke does to a mapping) · **Owner:** kernel session · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Main task:** — · **Constitution:** MC-2.6, MC-6.8, MC-12.4

From ASR's request with LNK and DRV (`KRN/2026-10-22T1606Z-LNK-a-lent-buffer-dropped-during-the-call.md` on `internal-dev-messages`). The maintainer's word, 2026-10-22: "`wm` must not fall in such cases".

## Problem

- **What happened on the MacBook Pro.**
  - `wm` called the shell's `net-counters` with a lent buffer (`wire::call_buffer`) and gave up when its timeout passed.
  - `call_buffer` then revoked the lease, as it must: the server may not change the buffer while the caller reads it, or after (MC-2.6).
  - The kernel took the server's mapping of the buffer away with the lease (`user_heap::revoke`).
  - The shell, still answering, wrote its reply there (`wire::reply_buffer`) and faulted: `VECTOR=14 ERROR=0x6`.
  - `init` restarted the shell, and `wm`'s screen went with it.
- **The general case.** Any server that answers a buffer call after its caller gave up faults the same way. The caller decides when that happens, so one slow answer is enough for a caller to stop a service (MC-6.8).

## Plan

1. **The rule.** When a lease is revoked, the kernel puts, in place of each page the borrower mapped from it, one zeroed page of the borrower's own.
   - Reads give zeros, and writes land in that page.
   - The owner's memory is out of reach exactly as before.
   - The mapping stays until the borrower frees it, which releases the page.
2. **The cost.** One page per mapping, charged to the borrower and its payers like its page tables. A borrower with no room left in its heap's quota loses the mapping as today: its next access faults, and `faults` shows it. A device mapping, and a task that has ended, get no page either.
3. **On every architecture.** A page table entry may be invalid for a moment while it is replaced (aarch64's break-before-make). A fault at an address that is mapped with the access asked by the time the kernel looks is retried, not fatal.
4. **ABI 17.**
   - `docs/api` says what a revoke does to a borrower's mapping.
   - The `isolation` cases that read a revoked lease (`v`, `l`) check the new rule: the borrower reads zeros, not what the owner wrote after the revoke.
5. **`wire`.** A server's reply to a caller that has gone fails with an error. Generated servers go on to the next request.

## Acceptance criteria

- [ ] Kernel host tests: a revoked mapping reads zeros and takes writes; freeing it releases the page; a borrower without room loses the mapping.
- [ ] The `isolation` suite (x86; its adversary program is x86 only): after a revoke the borrower reads zeros, never the owner's later data, and writes without a fault; a child that keeps reading a revoked lease goes on.
- [ ] ASR's suite (500-ASR-0012): a caller that gives up at different moments, the server still running and answering the next caller, on x86 and aarch64.
- [ ] `docs/api` and `docs/profile` state the rule.

## Related

500-ASR-0012 (ASR's suite, not in `main` yet), `libmind/src/idl/wire.rs` (`call_buffer`, `reply_buffer`), `kernel/src/user_heap.rs` (`revoke`).
