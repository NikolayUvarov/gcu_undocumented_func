# 750-KRN-0192 — `COMPOSITOR_PULL` names the task whose screen is in front (ABI 26)

**Type:** kernel, ABI · **Owner:** `KRN` · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [750](750-effector-agent.md), for [750-KRN-0193](750-KRN-0193-the-program-in-front-offers-places-for-the-marks.md) · **Roadmap:** track G · **Constitution:** MC-3.11, MC-10.2, MC-12.4

**Asked by:** EFF on 2026-10-24, in `ALL/2026-10-24T1654Z-EFF-a-request-the-program-in-front-offers-a-place-for-the-marks.md` on `internal-dev-messages`, which passes on the maintainer's choice of that day. EFF's word that it waits: `KRN/2026-10-30T1415Z-EFF-the-gateway-calls-its-updater-client-and-the-marks-request.md`.

## Problem

- **What 750-KRN-0193 needs.** The program in front offers the compositor places for the marks, and an offer counts only while that program's screen is in front.
- **What the compositor knows today.** `COMPOSITOR_PULL` hands it the screen in front, as memory, and says whether it changed. It does not say whose screen that is.
- **Why the screen's address is not enough.**
  - A new task may be given the frames an ended one had.
  - The pull then reports no new screen, so the compositor would keep the old task's offer.

## Plan

- **`COMPOSITOR_PULL` returns the PID of the screen's task in the mailbox's `arg2`,** where other calls return a second value, on every pull but a release. It returns 0 when no screen is in front.
- **Nothing else changes in the call:** its result, its modes (`COMPOSITOR_FRAME`, `COMPOSITOR_RELEASE`) and the hold.
- **`libmind::dev`:** `compositor_pull` and `compositor_pull_frame` return the frame state and that PID.
- **ABI 26:** `ABI_VERSION` and the handoff step, as for every ABI change (MC-12.4). PIDs are never reused, so a PID names one task for the whole boot.
- **Why the kernel says it:** the focus is the kernel's. Any other source, such as the shell's `fg`, could disagree with it after a Ctrl+Z or a program's end.

## Acceptance criteria

- **Host tests:** pass whole.
- **QEMU, x86 and aarch64:**
  - the compositor logs the PID of the screen in front when it changes (`[COMPOSITOR] SCREEN OF PID <n> IN FRONT`), at most its `LOG_LINES`;
  - the `display` checks see the shell's PID first, then a program's in front, then the shell's again after Esc.
- **No other suite changes.**

## Related

[750-KRN-0193](750-KRN-0193-the-program-in-front-offers-places-for-the-marks.md), [800-KRN-0156](800-KRN-0156-the-remote-marks-in-the-compositor.md), [750-APP-0110](750-APP-0110-wm-offers-its-top-bar-for-the-marks.md), [750-EFF-0015](750-EFF-0015-the-agents-mark-on-the-screen.md).
