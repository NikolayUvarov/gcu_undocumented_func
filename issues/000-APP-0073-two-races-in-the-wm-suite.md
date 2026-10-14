# 000-APP-0073 — Two races in the `wm` suite: a title written after adoption, and Turn off's Enter

**Type:** tools (`wm`, tests) · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** — · **Roadmap:** track G · **Constitution:** MC-12.2

**Recorded by:** the kernel track (KRN), 2026-10-14, in `requests-APP.md`, from its gates of 63ef648 on i9 and 62a2624 on un-rizen-service. Numbered by the tools track.

## Problem

Two checks of the `wm` suite fail now and then, on a slow or busy guest.

- **A window adopted before its title is written.**
  - `libmind::windowed` calls `create`, then `surface`, and only then writes the title into the surface.
  - `wm` adopts a window as soon as the broker can lend its surface, and logs it once (`[WM] WINDOW … ""`).
  - `wm` takes the title on its next frame, so the frame shows it. But the log line keeps the empty title.
  - `open_shell` in tests/qemu_smoke.py waits for a window line ending `"shell"`, so it timed out.
- **Turn off's Enter waits for a line the shell holds.**
  - `every_entry_reacts` presses Enter on "System>Turn off" and waits for one `[WM] MODE=` line.
  - The shell asks in its window, and meanwhile passes `wm`'s lines on no more. The line came only after the test's answer, which waited for it.

## Plan

- `wm/src/main.rs`: a title written after the window was adopted is logged once, `[WM] TITLE <id> "<title>"`.
- tests/qemu_smoke.py:
  - the shell's window is found by its `WINDOW` line or its `TITLE` line;
  - Enter on Restart and Turn off waits for no state line. The test reads the question from the screen, answers no, and then waits for `wm`'s reaction line.

## Acceptance criteria

- The `wm` suite passes. Its `shell_window` and `every_entry_reacts` find the shell's window by either line, and the power entries no longer wait for a held line.
- `[WM] TITLE` appears only for a window adopted with an empty title.

## Related

[000-APP-0065](../issues-done/000-APP-0065-poweroff-shutdown-and-turn-off-in-wm.done) (Restart and Turn off), [211-APP-0040](../issues-done/211-APP-0040-the-shells-commands-in-console.done) (the shell's window), [000-APP-0056](../issues-done/000-APP-0056-every-entry-of-wms-menu-reacts.done) (every entry reacts).
