# 211-APP-0057 — `top` and `free` show the machine's memory, not only the kernel's arena

**Type:** tools · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** in progress (done in QEMU; the MacBook Pro's run left) · **Blocked by:** — · **Main task:** [211](211-intel-pc-from-a-sata-ssd.md) · **Roadmap:** track G · **Constitution:** MC-10.2

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

## Progress

- **`idl/sysinfo.wit` 4.1** adds `pool`, the frame pool (`frames`, `frames-free`), as a new function. Appending the fields to `memory` would have changed its layout, and the decoders refuse trailing bytes. `sysmon` serves it.
- **`top`'s header** has two memory lines:
  - `Mem`: the frame pool, used of all and free;
  - `Kern`: the kernel arena, its largest free block, tasks and endpoints.

  A `sysmon` before 4.1 leaves `Mem` saying it is not known.
- **The shell's `free`** begins with `MEMORY: <n> MIB, <n> MIB IN USE, <n> MIB FREE (THE FRAME POOL PROGRAMS RUN IN)` and `FRAMES=… FRAMES_FREE=…`. The kernel arena follows as `KERNEL ARENA: ARENA=…`, with its categories as before. The help line says so.
- **`load` and `memmap`** keep their names: "kernel arena" there is the arena.
- **Docs:** `docs/tools` (EN, RU) and `docs/idl` describe it.
- **Host tests:** `tests/monitor_host.rs` checks both lines of `top` (7768 MiB, 89 MiB in use, as the MacBook Pro's pool was).
- **The boot suite's frame-pool check** compares `free`'s first line and `top`'s `Mem` bar with the pool's size.
- **Checked:**
  - in QEMU with 512 MiB, `free` and `top` show 386 MiB of memory (38 MiB in use) and the arena apart;
  - the local gate's host tests and every x86 QEMU group passed (2026-10-10), the 6 GiB machine's `normal` suite among them.
- **Left:** the MacBook Pro's 7.6 GiB in `top` and `free`.

## Related

[requests-APP.md](requests-APP.md) (where it was recorded).
