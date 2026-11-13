# 000-NET-0077 — The take-down of a run-time question read across the screen's width

**Type:** network (a test of the run-time question) · **Owner:** `NET` · **Priority:** P2 · **Status:** in progress · **Blocked by:** — · **Main task:** — ([000-NET-0049](../issues-done/000-NET-0049-a-flow-grant-asked-at-run-time.done)) · **Roadmap:** track D · **Constitution:** MC-12.2

## Problem

111-NET-0076's run on i9 (2026-11-13, at c57928a1) failed "x86: network, TLS and Effector" (1197 s) in the `net` suite's `browse_check`:
- **The check:** `netcheck ask:11.0.2.80:81:3` asks a question at run time and ends after 3 s. The shell takes the question down with `(GONE: ITS PROGRAM ENDED, OR NO ANSWER CAME WITHIN 60 S AND THE BROKER REFUSED IT)` and, when that comes after the command's prompt, shows the prompt again. The check waited up to 10 s for `MIND> ` after the text `(GONE: ITS PROGRAM ENDED`.
- **What the log showed:** the line, then `MIND> `, but the line was cut by the screen's width inside that text: `…ENDE` at the end of one line, `D, OR NO ANSWER …` at the start of the next. The text the check waited after never came whole, so the wait ran out with the prompt already there. Read from the run's log (`logs/n76b/i9-01` on i9).
- **Why there:** where the line is cut depends on where the cursor stood when the shell wrote it, which the lines before it decide. It is not a property of i9; the same could happen on any machine.

## Plan

The check reads the log since the command, its lines joined and ANSI codes removed (`joined()`), and waits there for `MIND> ` after the take-down's text, up to 10 s as before.

## Acceptance criteria

- The `net` suite passes on x86 and aarch64.

## Progress

**Made (2026-11-13)** on `claude/111-NET-0076-nts-refused-race`, with 111-NET-0076: `tests/qemu_smoke.py` `browse_check` and `joined()`.

## Related

[000-NET-0060](../issues-done/000-NET-0060-the-run-time-question-read-whole-from-the-log.done), [000-APP-0120](../issues-done/000-APP-0120-the-shell-asks-the-user-about-a-programs-flow.done), [111-NET-0076](111-NET-0076-two-time-checks-i9-showed.md).
