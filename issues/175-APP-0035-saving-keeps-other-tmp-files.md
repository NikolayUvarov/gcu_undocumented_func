# 175-APP-0035 — Saving does not destroy an existing `<name>.tmp`

**Type:** tools (`edit`, `fm`) · **Owner:** tools track (`APP`) · **Priority:** P2 (P1 in the audit) · **Status:** open · **Blocked by:** — · **Main task:** [175](175-audit-2026-10-09.md) (audit A07) · **Roadmap:** track G · **Constitution:** MC-4.8

Numbered from the kernel track's request in `requests-APP.md`, which routed the 2026-10-09 audit at the maintainer's decision. That file went once every request in it was numbered.

## Problem

Audit finding A07 ([audit](../issues-audit/2026-10-09-repository-audit.md), [assessment](../issues-audit/2026-10-09-repository-assessment.md)), confirmed.

- `edit` saves through `File::create`, which opens with `MODE_TRUNCATE`.
- `fm`'s editor calls `create(&temporary, true)`, also mapped to `MODE_TRUNCATE` (`fm/src/main.rs`).
- Both stage into the fixed name `path + ".tmp"`. Saving `x` therefore destroys an unrelated `x.tmp`, and two editors saving one file collide. `MODE_NEW` exists (`libmind/src/fs.rs`).

## Plan

- The staging file is created exclusively (`MODE_NEW`). On a collision another name is chosen (`x.tmp1`, `x.tmp2`, …), and the save keeps track of the one it made.
- Only that file is renamed over the target or removed after a failure.

## Acceptance criteria

- **Host tests:**
  - an existing `x.tmp` survives a save of `x` byte for byte;
  - two saves of one file use two staging files;
  - a failed save removes only its own staging file.
- `issues-audit/repro/fm_repro.py`'s A07 part becomes the regression test, with its assertion inverted.
- **The `edit` suite:** a save next to an existing `.tmp` file leaves that file as it was.

## Related

[175](175-audit-2026-10-09.md), [067](../issues-done/067-editor.done), [068](../issues-done/068-fm-write-df-fsck.done), [175-APP-0036](175-APP-0036-fm-move-keeps-source-until-flushed.md).
