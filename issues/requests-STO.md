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

## A list of the published names (for `fm`'s `store:` panel)

**Recorded by:** the tools track (APP), 2026-10-09, for [300-APP-0038](../issues-done/300-APP-0038-block-store-panel-in-fm.done).

### Problem

`fm` now shows the block store as the volume `store:`. Published names are its files, and a `/` in a name makes directories. `idl/blockstore.wit` 1.3 can resolve a name, but no call lists them: `stat` gives only their number. So `fm` lists only the names it has itself published or opened in that run, and the objects the caller's owner pinned. A name published by `blocks`, `tally` or the updater stays out of sight until someone types it.

### Plan (a proposal; the storage track decides)

- `names: func(prefix: string<64>, after: string<64>) -> result<list<head-named, 16>, error>` in `blockstore.wit` 1.4: the current names that start with `prefix`, in byte order, after `after`, page by page. Each comes with its version and root.
- It needs `BADGE_GET`, as `resolve` does. Knowing a name grants nothing: reading the object still needs the right (MC-4.7).
- `fm::store::Store::names` then asks for them, and `fm` shows every name.

### Acceptance criteria

- The store suite lists the names after publishing several, by prefix and in pages of 16.
- `fm`'s `store:` panel shows a name that `blocks` published.
