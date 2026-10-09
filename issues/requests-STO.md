# Requests for the state and recovery track (STO), not numbered yet

**Owner:** state and recovery track · **Status:** open

The STO track numbers its own tasks (`NNN-STO-MMMM`), so requests from other tracks wait here. It turns each into a task and removes it from this file.

## The block store acknowledges a block it erased (audit A05, main task 175)

**Recorded by:** the kernel track (KRN), 2026-10-09, routing the 2026-10-09 audit ([175](175-audit-2026-10-09.md)) at the maintainer's decision.

### Problem

Audit finding A05 ([audit](../issues-audit/2026-10-09-repository-audit.md), [assessment](../issues-audit/2026-10-09-repository-assessment.md)), confirmed.
- `pass(true)` erases unretained records as it scans (`blockstore/src/store.rs`, line 434), but drops them from the index only after the whole scan (lines 527–533).
- A read failure returns `Device` midway (line 417), and the erased records stay indexed.
- `put`'s fast path (line 887) then returns the CID without writing. That breaks `put`'s own contract and MC-4.4.

### Priority

P2 while the store runs on a RAM disk. P1 before it gets a durable medium (stage IV).

### Plan and acceptance (from the audit)

- The index and the free space stay consistent with every erase that completed. Or the store refuses puts until a full rescan succeeds after an indeterminate failure.
- **Tests.** Failures are injected throughout sweep and erase, then puts are retried on the same instance. Every put that succeeded must be readable under the medium's declared contract.
- `issues-audit/repro/blockstore_repro.rs` becomes the regression test.
