# 211-APP-0057 — `top` and `free` show the machine's memory, not only the kernel's arena

**Type:** tools · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [211](211-intel-pc-from-a-sata-ssd.md) · **Roadmap:** track G · **Constitution:** MC-10.2

Numbered on 2026-10-10 from `requests-APP.md`, where the kernel track recorded it.

**Recorded by:** the kernel track (KRN), 2026-10-10, at the maintainer's question after a run on the MacBook Pro: "why is the available memory shown as 64 MB, when the computer has gigabytes?"

## Problem

- **The memory bar shows the kernel arena.** `top`'s memory bar and line (`monitor/src/top.rs`, `m.used` of `m.arena`) show the kernel arena: the 64 MiB of kernel structures (tasks, endpoints, capability tables). So it reads "2 MB of 64 MB" as if that were all the memory.
- **The machine's memory is not shown, though the kernel sees it.**
  - On the MacBook Pro the firmware's map has 7.6 GiB of conventional memory, 5.7 GiB of it above 4 GiB.
  - The kernel's frame pool is `7768 MiB, 7679 MiB free` (`hw0001.txt`, "The kernel's choices").
  - `StatMemory` carries it already: `frames` and `frames_free`, in bytes. That pool holds the programs' images, stacks, screens, heaps and objects, so about 89 MiB were in use, not 2 MB.
- **`free` prints it on a second line** (`FRAMES=… FRAMES_FREE=…`), after `ARENA=…`, so it reads the same way.

## Plan

- **`top`:** the first memory bar is the machine's memory: `frames - frames_free` of `frames` (named "memory" or "RAM"). The arena gets a second, smaller line named "kernel arena".
- **`free`:** the frame pool first, as "memory", then the arena as "kernel arena". `load`'s "kernel arena" series keeps its name.
- **`sysmon`'s and `wm`'s summaries, if they show memory:** the same.
- **`docs/tools` (EN, RU):** what the frame pool and the arena are.

## Acceptance criteria

- On QEMU with 512 MiB, `top` and `free` show about 400 MiB of memory with what is in use, and the arena separately.
- On the MacBook Pro they show about 7.6 GiB.

## Related

[requests-APP.md](requests-APP.md) (where it was recorded).
