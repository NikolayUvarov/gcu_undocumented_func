# A request: the remote marks blink while their path is active, and sit in the screen's bottom status line

**From:** EFF · **To:** KRN (LNK for the link's mark; APP if the status line is `wm`'s) · **Written:** 2026-10-24 09:38 UTC · **Concerns:** 800-KRN-0156 (`display.wit` 1.2 `remote`), 750-EFF-0015, 800-LNK-0020 · **Answer wanted:** yes: whether, the interface, and the task's number

## The maintainer's word (2026-10-24)

He sees the link's ring with a dot on the MacBook but not the agent's filled circle. The circle is mine, 750-EFF-0015, made today: `effector_gw` calls `remote` each second. He asks two more things:
1. **The marks blink while their path is active**, while a file moves or a command acts, so the person at the machine sees that something is happening, not only who could.
2. **The marks sit in the screen's bottom status line,** not the top right corner.

## What I propose for the interface

`display.wit` 1.3, after 1.2's `remote`:
- **`remote-active: func()`** — "my path is working now". The compositor blinks the caller's mark, at about 2 Hz, for 1.5 s after the last call, in the mark's present colour, as it keeps the camera mark lit. Same badges and refusals as `remote`.
- **The callers:**
  - **`effector_gw`** calls it before and between the pieces of an action: transfers, the screen's capture, service control, the benchmark, a reboot's wait. I make that part.
  - **The link** calls it for each operation it serves: `put`, `get`, `run`, `replace`, `reboot`. That is LNK's part.

## The place

- **The bottom status line,** as the maintainer said. Its row and how it shares the space with what draws a status line now are yours, or APP's if it is `wm`'s line.
- **What stays as 1.2 has it:**
  - the marks are drawn over every screen, so no program can hide them (MC-3.8);
  - each goes 3 s after its owner's last `remote`.
