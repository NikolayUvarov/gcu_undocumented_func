# 000-APP-0154 — `wm`'s Run window (Alt+R) keeps a history of what was entered

**Type:** tools (`wm`) · **Owner:** tools track (`APP`) · **Priority:** P2 (the maintainer's word, 2026-11-21, passed on by DRV) · **Status:** in progress (done in QEMU on x86, in `main` since round 169, 2026-11-22; the Ryzen's run left) · **Blocked by:** — · **Main task:** — · **Roadmap:** track G · **Constitution:** MC-12.1

**Recorded by:** the tools track, 2026-11-21, from DRV's message `APP/2026-11-21T1325Z-DRV-request-wms-run-window-keeps-a-history-of-what-was-entered.md` on `internal-dev-messages`. It passes on the maintainer's word after `camera -c 0` and `camera -c 1` were started through Alt+R on the Ryzen: «окно Alt+R (запуска программ) должно поддерживать историю введённых команд, поставь задачу треку APP».

## Problem

- **The Run window forgets each line once it runs.** Running a program again with other arguments means typing it whole again.

## Plan

- **↑ and ↓** step through the lines entered before, newest first, as in the shell (`libmind`'s `History`). The line being typed comes back after the newest, and a line brought back can be edited before it runs.
- **What is kept:**
  - each line that runs, unless it repeats the newest, 16 at most;
  - in `data/wm-run.txt`, one a line, the oldest first, after a comment line;
  - read at `wm`'s start, so the next `wm` and the next boot have them.
- **Not the shell's history.** Each console of the shell keeps its own in memory; this one is `wm`'s Run window's alone. The menu's entries are not lines entered and are not kept.
- **The dialog says so** once there is a history: `Program and arguments (↑ ↓: earlier ones):`.

## Acceptance criteria

- **Host tests** (`tests/wm_host.rs`):
  - ↑ and ↓ through two lines and back to the one being typed;
  - a repeat not kept twice, and the oldest stays at its end;
  - a line brought back and edited;
  - Esc, and the next window from the newest again;
  - the file written and read back, with a comment, a blank line and a control character left out, 16 kept of 40;
  - the dialog's words.
- **The `wm` suite in QEMU:** a line kept in `data/wm-run.txt` once it ran; ↑ and Enter run it again.
- **Docs:** `docs/tools/README.md` and `README_RU.md`.

## Progress (2026-11-21)

- **Done** as planned:
  - `Wm::runs`, a `History` of 16;
  - `runs_text` and `runs_read`;
  - the load at `wm`'s start and the write after a new line, `[WM] RUN HISTORY: N LINES KEPT IN data/wm-run.txt` in its log.
- **Tested** in this container, x86_64, 4 CPUs: `tests/wm_host.rs` (49 tests) and the `wm` suite pass. In the suite, `clock` run from the Run window is kept in `data/wm-run.txt`, and ↑ with Enter starts it again.
- **Left:** the Ryzen's run, where the maintainer asked for it (MC-12.1: QEMU's screen and keyboard are not the Ryzen's).

## Related

[211-APP-0040](../issues-done/211-APP-0040-the-shells-commands-in-console.done) (the shell's commands in `wm` and `console`), [158-APP-0147](158-APP-0147-switching-between-cameras-in-camera-and-wm.md) (`camera -c N`, the lines that asked for it).
