# 213-DRV-0100 — `display`: the second screen's vertical blank, if the mirror tears

**Type:** driver (`display`) · **Owner:** `DRV` · **Priority:** P3 · **Status:** open (2026-11-22) · **Blocked by:** the maintainer's look at the mirror (does it tear?); a display.wit 1.4 from KRN · **Main task:** [213](213-amd-ryzen-pc.md) · **Roadmap:** the drivers track · **Constitution:** MC-12.1, MC-12.3, MC-12.4

## Problem

display.wit 1.3 (213-KRN-0272) has no vertical blank. The compositor draws into the second screen when its own frame
comes, so a changed span may be drawn while pipe 1 scans it, and the mirror may tear. Whether it does on the Ryzen is
not known yet.

## Plan

- **Only if the mirror tears:**
  - `display` gives OTG1's vertical blank: its frame count, or the vertical interrupt's status, polled. The display
    engine's interrupt is not `display`'s.
  - DRV then asks KRN for a display.wit 1.4 that carries it (a request in `issues/requests-KRN.md` or a message, MC-12.4).
- **If it does not tear,** this task closes with that observation and no change.

## Acceptance criteria

- [ ] The maintainer's look at the mirror while DP-2's screen changes: tearing or not.
- [ ] If tearing: the request to KRN recorded, and `display`'s side ready with host cases.

## Related

[213-DRV-0092](213-DRV-0092-display-mind-cores-picture-on-the-second-output.md), [213-KRN-0272](213-KRN-0272-a-second-screen-for-the-compositor.md).
