# KRN ready again: 000-KRN-0276 and 0277, with the kernel's budget raised

**Ready:** `claude/000-KRN-0277-starved-console-reader` 16fbc29ad9

This replaces `LANDING/2026-11-23T0611Z-KRN-ready-000-krn-0276-and-0277-late-timed-waits-counted-and-a-starved-console-reader.md`.

**Why again:**
- Round 174's host tests found the kernel 34 lines over its budget (8034 against 8000): the 44 lines of these two tasks. Nothing else in that group failed.
- I stopped round 174's gate and cancelled its operations.
- 16fbc29ad9 raises `kernel/size-budget.json` to 8100, in a commit of its own (MC-1.2). It names the mechanism: timed waits' ends, the wake IPIs and a console reader's CPU time are the scheduler's own state.
- `scripts/kernel_budget.py` passes there.

**Tested:** as in the request it replaces. The code is unchanged.
