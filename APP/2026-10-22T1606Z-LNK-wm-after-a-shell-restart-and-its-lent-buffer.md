# A request: `wm` keeps its screen when the shell restarts, and keeps its lent buffer until the answer

**From:** ASR (with LNK and DRV) · **To:** APP · **Written:** 2026-10-22 16:06 UTC · **Answers:** — · **Concerns:** `wm`, the shell's start, `shell.wit` `net-counters` (000-APP-0096); my report of the same hour; 500-ASR-0012 · **Answer wanted:** yes: your task's number

## Why: the maintainer's word, 2026-10-22

"`wm` must not fall in such cases."
- **What the maintainer saw.** When the shell faulted on the MacBook Pro (my report), `init` restarted it, `wm` (PID 56) kept running, and its screen was gone: the new shell showed its own console.
- **KRN is asked** for the root: a server answering into a buffer its caller dropped must go on. Your part makes `wm` safe whichever way that goes.

## What is asked

1. **A restarted shell finds the `wm` that runs and gives it back the screen,** or starts it again if it is gone, so the maintainer keeps `wm` after any shell restart.
2. **`wm` keeps the buffer it lent the shell until the answer comes,** rather than dropping it when its `with_timeout` ends.
   - A late answer is then written into memory that is still there.
   - The next call waits for it, or uses a second buffer.
3. **While KRN decides,** the shell's answers into a lent buffer could check the call is still open, if `wire` gives it a way to.

## Tests

- **Yours:** a QEMU check that kills the shell (`kill` from a test) and finds `wm` back on the screen.
- **ASR's 500-ASR-0012:** the caller that gives up.
