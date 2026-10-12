# 000-APP-0064 — `wm`'s background: the time with seconds, the date, the CPU load and the free memory in one column

**Type:** tools · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** — · **Roadmap:** track G · **Constitution:** MC-11.5

**Recorded by:** the maintainer, 2026-10-12: "the date, the time and the CPU on the background need fixing: all aligned on the left, so that the time, the date and the CPU are at one level. Also add the free memory there. And add seconds to the time."

## Problem

`wm`'s background (000-APP-0047) shows its information in a block at its place, by default in the bottom right corner. There each line is aligned on the right of the block, and the lines are of different widths:

- the time, `HH:MM`, in large digits;
- the date;
- the CPU line and its graph.

So the lines start at different columns. The time has no seconds, and the free memory is not shown.

## Plan

- **One column.** Every line starts at the block's left edge, wherever the block is.
- **Seconds.** The time is `HH:MM:SS`. The CPU graph is as wide as the time.
- **The free memory.** A line `MEM 336 MiB FREE` (GiB from 10 GiB on) under the CPU graph. It shows the frame pool's free memory, which `wm` reads once a second with `sysinfo`'s `pool` (211-APP-0057).
- **The configuration.**
  - `show` takes `memory`, and it is in the default.
  - A `wm.conf` that never names `memory` was written before it could be chosen, so it shows the memory too.
  - Settings' background page gets the row "Free memory".

## Acceptance criteria

- `tests/wm_host.rs`:
  - every line starts in the block's first glyph cell, the block in the bottom right corner too;
  - the time has eight characters;
  - the memory line is drawn;
  - `show` reads and writes `memory`;
  - an old `show` line without it still shows the memory;
  - Settings' new row ticks it.
- The QEMU `wm` suite: the default background line names `memory`, and Settings' rows walk includes "Free memory".
- `docs/tools` (EN and RU) name `memory` among `show`'s words.

## Related

- 000-APP-0047, 000-APP-0050: the background and its information.
- 211-APP-0057: the frame pool in `sysinfo` (`pool`).
