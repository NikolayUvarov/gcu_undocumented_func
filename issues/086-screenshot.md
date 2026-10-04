# 086 — `screenshot`: the screen as a BMP file

**Type:** tool · **Owner:** tools track · **Priority:** P2 · **Status:** open · **Blocked by:** — ([151](../issues-done/151-shell-grant-slots-13-15.done) done) · **Roadmap:** track G, T3 · **Constitution:** MC-2.6

## Problem

Only the compositor reads task screens (display privilege); there is no way to save what is on screen ([docs/tools](../docs/tools/README.md) §2.3).

## Plan

- `idl/display.wit` 1.0, served by `compositor` on the endpoint `init` gives it (151): `capture() -> result<own<memory>, error-code>` — a sealed read-only copy (SHARE_RO) of the focused screen as `0x00RRGGBB` pixels, plus `mode() -> mode` (width, height, stride).
- `screenshot [file]` (shell command, default `ram:screen-NNN.bmp`): asks the compositor through the shell's display client (slot 15, 151) and writes a 24-bit BMP with the user's file client.
- Host test of the BMP writer; QEMU `tools` suite: a screenshot of `view` matches the screen the harness reads back (pixel colours of the first text line).

## Acceptance criteria

- The tests above pass; README and `docs/tools` describe `screenshot`.

## Related

[151](../issues-done/151-shell-grant-slots-13-15.done).
