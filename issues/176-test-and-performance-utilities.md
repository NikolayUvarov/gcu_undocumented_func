# 176 — Utilities to check the system and measure it: `check`, `bench`, `kbench`

**Type:** main task (kernel track, with the tools track for the menu) · **Owner:** kernel session · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Roadmap:** Assurance, track A · **Constitution:** MC-12.1 (evidence names its configuration), MC-12.2 (tests are evidence, not proof)

## Problem

The maintainer asked (2026-10-10) for three kinds of utilities:

- **Test runs:** a check of what has been done.
- **Component performance runs.**
- **Kernel performance runs:** a clear display on the screen for the user, and a full log to analyse.

They must be at hand from the menu and from the command line, and between them check everything.

Today the QEMU suites check the system only from outside, on the build machine. On a real machine (the MacBook Pro, a PC) the only evidence is the boot log, the hardware report and what the maintainer tries by hand. `netbench` and `memtest` measure one thing each.

## Plan

One crate, `bench/`, with three console programs that share their tables, statistics and log writer:

- [176-KRN-0062](176-KRN-0062-kbench.md) — **`kbench`**, the kernel's performance:
  - system calls, the clock, IPC round trips between two processes, with and without a lent page;
  - capability and endpoint operations;
  - page allocation;
  - timer accuracy;
  - process start and exit.
- [176-KRN-0063](176-KRN-0063-check.md) — **`check`**, a self-test of what is done, component by component:
  - each check passes, fails with its reason, or is skipped with its reason (no such device, not granted);
  - a summary at the end.
- [176-KRN-0064](176-KRN-0064-bench.md) — **`bench`**, the components' performance:
  - files on each volume;
  - the services' round trips;
  - hashing;
  - the camera's frame rate.

What they have in common:

- **On the screen.** A table of at most 80 columns (box lines, bars in eighths of a cell, ✓ ✗ ○), readable in the shell and in `console` in `wm`.
- **The log.** The full log goes to `log:` on the boot disk (`ram:` without it), named with the boot's number: every sample's statistics, histograms, the machine (CPUs, memory, the kernel's build), each check's details. The last line on the screen names the file.
- **Starting them.** From the shell (`check`, `bench`, `kbench`, with the group names to run a part) and from `wm`'s menu: they appear under "Other" until the tools track gives them a category (requested in `requests-APP.md`).
- **Testing them.** A QEMU suite runs all three on x86 and aarch64 and checks their summaries and logs.

## Acceptance criteria

- The three programs run from the shell and from `wm`'s menu, on QEMU (x86, aarch64) and on the MacBook Pro.
- Each prints its table and a summary, and writes its log.
- `check` passes on QEMU with the devices of the suites, and says what it skipped and why.
- The numbers of `kbench` and `bench` on the MacBook Pro are recorded in the profile's evidence, with the configuration they belong to (MC-12.1).

## Related

`netbench` (106), `memtest`, `monitor` (`top`, `load`), the hardware report (174-KRN-0038).
