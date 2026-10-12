# 211-KRN-0095 — `record` on a Retina screen: the compositor's quota holds the copies of the screen

**Type:** `init` (the compositor's memory quota) · **Owner:** kernel session · **Priority:** P1 · **Status:** in progress (made; the MacBook Pro's run left) · **Blocked by:** — · **Main task:** [211](211-intel-pc-from-a-sata-ssd.md) · **Constitution:** MC-3.13, MC-12.1

## Problem

On the MacBook Pro (2026-10-12, the maintainer's photo) `run record -r 10 -t 30 data/demo.avi` stopped at once: "0 FRAMES … 232 BYTES (STOPPED: no memory for the copy)". The maintainer asked whether the disk was full. It was not.

- `record` asks the compositor for each frame (`display.wit` `capture`). The compositor answers with a sealed copy of the screen, made from its own memory.
- `record` keeps two such copies at once: the frame it codes and the one before, against which it codes only the rows that changed.
- The compositor's quota was the default 16 MiB and one screen ([211-KRN-0018](../issues-done/211-KRN-0018-the-compositors-quota-fits-the-screen.done)). That one screen is its shadow copy of the framebuffer.
- At 2880 × 1800 a screen is 20 MiB, so a copy did not fit. At QEMU's 1280 × 800 (4 MiB) three screens fit in the default, which is why the suites passed.

## Plan

The compositor's quota is the default and three screens: its shadow copy and the two copies `record` holds (76 MiB on the MacBook Pro). `screenshot` holds one copy at a time, so it fits too.

## Acceptance criteria

- **QEMU:** the `vfs` suite's recording and the `wm` suite's window recording pass as before.
- **The MacBook Pro:** `record -r 10 -t 30 data/demo.avi` writes its frames; the log says how many it coded and how long each took.

## Related

[211-KRN-0018](../issues-done/211-KRN-0018-the-compositors-quota-fits-the-screen.done), [000-KRN-0093](000-KRN-0093-pictures-and-video-for-the-showcase.md) (the pictures and video for the project's page), `record/src/main.rs`.
