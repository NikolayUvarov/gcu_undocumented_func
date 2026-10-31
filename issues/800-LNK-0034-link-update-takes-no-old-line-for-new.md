# 800-LNK-0034 — `link_update.py` takes no old line of the log for a new one

**Type:** tool · **Owner:** `LNK` · **Priority:** P2 · **Status:** in progress (2026-10-31) · **Blocked by:** — · **Main task:** [800](800-mind-link.md) · **Constitution:** MC-12.1

## Problem

- **What happened.** The MacBook Pro's slot update of 2026-10-31 (release 202610310825, `main` 1c68b668) put all 107 files and the channel. `link_update.py` then ended `exit 1` on this line:
  ```
  68816.683 updater: [UPDATER] CHECK: REFUSED: Older
  ```
- **That line was old.** The updater was fetching the release at 71483 s, and the Mac restarted into slot B on trial and confirmed it by itself. The update succeeded; only the tool said it failed.
- **Why.** `follow` reads the whole log again and again, and `fresh` finds the last lines of the previous read (the anchor) in it.
  - Without the anchor, it took the read for the next boot's and every line as new, an old refusal among them.
  - On the Mac a read is a day's log, minutes long over the cable, and that read had lost the anchor.

## Plan

1. **`fresh` without the anchor goes by the uptimes,** each line's first word:
   - all lines when the log begins again (its first line younger than 60 s) and ends before the latest uptime read: the next boot;
   - none when it ends before that otherwise: a stale read, after which the anchor and the latest uptime stay as they were;
   - else the lines after the latest uptime, and those of the same uptime that were not in the anchor.
2. **With the anchor,** as before.
3. **`tests/link_update_test.py`:** the Mac's case (a stale read with an old refusal, then the next boot's confirmation) and a line of the same uptime as the last read.

## Acceptance criteria

- [x] `tests/link_update_test.py` passes with both cases.
- [x] The CI group "x86: the updater and an update through the link" passes.
- [ ] The next slot update over the cable ends `exit 0` on a confirmed slot.

## Related

[800-LNK-0033](800-LNK-0033-a-machine-busy-writing-its-disk-ends-no-put.md), [351-KRN-0141](../issues-done/351-KRN-0141-an-update-through-the-link.done) (the tool).
