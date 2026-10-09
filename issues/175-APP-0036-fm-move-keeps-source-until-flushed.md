# 175-APP-0036 — `fm` keeps a move's source until the destination is on its medium

**Type:** tools (`fm`) · **Owner:** tools track (`APP`) · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Main task:** [175](175-audit-2026-10-09.md) (audit A08) · **Roadmap:** track G · **Constitution:** MC-4.8, MC-4.9

Numbered from the kernel track's request in `requests-APP.md`, which routed the 2026-10-09 audit at the maintainer's decision. That file went once every request in it was numbered.

## Problem

Audit finding A08 ([audit](../issues-audit/2026-10-09-repository-audit.md), [assessment](../issues-audit/2026-10-09-repository-assessment.md)), confirmed.

- **The order.** A move between volumes plans its removals after its copies, and `finish()` flushes the sources before the target (`fm/src/fm.rs`).
- **The lost error.** `Disk::flush` returns `()`, and `fm/src/main.rs` drops the error of `root.flush()`.
- **The result.** In a move between two durable volumes (`data/` to a USB stick), the source's removal can reach its medium before the destination's data. A later I/O error or power loss then loses the file, while `fm` reports it moved.

## Plan

- `Disk::flush` returns its error, and so does the editor's final flush.
- A move flushes each destination, and removes the source only after that flush succeeded.
- When writing or flushing fails, the source stays and `fm` shows the failure.

## Acceptance criteria

- **Host tests (`tests/fm_host.rs`):**
  - errors from the destination's write-back and flush;
  - moves of several files, retry and cancellation;
  - copy and save completion pass flush errors on;
  - no source is removed before its destination's successful flush.
- `issues-audit/repro/fm_repro.py`'s A08 part becomes the regression test, with its assertion inverted.

## Related

[175](175-audit-2026-10-09.md), [068](../issues-done/068-fm-write-df-fsck.done), [175-APP-0035](175-APP-0035-saving-keeps-other-tmp-files.md).
