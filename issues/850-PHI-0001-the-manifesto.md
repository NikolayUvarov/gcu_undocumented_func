# 850-PHI-0001 — The manifesto: one page on what is broken, what MIND Core builds instead, and how to join

**Type:** document (English and Russian) · **Owner:** `PHI` · **Priority:** P2 · **Status:** open (2026-10-21) · **Blocked by:** [850](850-the-cultural-code.md)'s questions on the hero and the adversary · **Main task:** [850](850-the-cultural-code.md) · **Constitution:** MC-12.3

## Problem

Nothing short and outward-facing says why MIND Core exists. The Constitution is written for the architect and the reviewer; the README's "Why MIND Core" is a story, not a position.

## Plan

- **The text.** One page, in `docs/culture/MANIFESTO.md` and `MANIFESTO_RU.md`, and linked from the top of the README.
  - **A diagnosis.** What is broken: ambient authority, drivers and file systems inside one address space, updates that cannot be checked or undone.
  - **An image of what MIND Core builds instead.**
  - **A call to join** (AGENTS.md, TRACKS.md).
  - **No technical vocabulary** beyond what a reader can follow.
- **The truth rule.** Each statement about the system either:
  - holds today, with a link to its evidence (a QEMU suite, a profile row, a measured number such as the kernel's size); or
  - is marked as a goal.
- **Not normative.** It does not quote the articles as promises, and it is not part of the Constitution.

## Acceptance criteria

- [ ] `docs/culture/MANIFESTO.md` and `MANIFESTO_RU.md` exist and fit on one page each.
- [ ] Every statement about the system links to its evidence or says it is a goal. A reviewer from another track checks this and signs it off in this issue.
- [ ] The README links to it.

## Related

[850](850-the-cultural-code.md), [850-PHI-0002](../issues-done/850-PHI-0002-the-canons-maxims-in-lore-1-6.done).
