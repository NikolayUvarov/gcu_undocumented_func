# 176-APP-0051 — A "Tests and performance" category in `wm`'s menu

**Type:** tools · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** in progress (the category is in; `check`, `bench` and `kbench` join it when the kernel track's `bench/` crate reaches `main`) · **Blocked by:** — · **Main task:** [176](176-test-and-performance-utilities.md) · **Roadmap:** track G, I · **Constitution:** MC-12.2

Numbered on 2026-10-10 from `requests-APP.md`, where the kernel track recorded it.

**Recorded by:** the kernel track (KRN), 2026-10-10, for main task [176](176-test-and-performance-utilities.md) at the maintainer's request.

## Problem

The kernel track is making three console programs in a new crate, `bench/`:

- `check`, a self-test of what is done;
- `bench`, the components' performance;
- `kbench`, the kernel's performance.

The maintainer wants them at hand from the menu as well as from the command line. With no category in `wm/src/menu.rs` (`CATEGORIES`), they land under "Other", next to `netbench` and `memtest`, which belong with them.

## Plan

- A category "Tests and performance" with `check`, `bench`, `kbench`, `netbench` and `memtest`, each started as `console <name>`, so its table shows in a window.
- Their lines in `help` and `docs/tools` (EN, RU) once the programs are in `main` (the kernel track writes their own pages).

## Acceptance criteria

The five programs are under that category in `wm`'s menu and run in a `console` window.

## Progress

- **The category.** `wm/src/menu.rs` has "Tests and performance" after Network, naming `check`, `bench`, `kbench`, `netbench` and `memtest`. Each runs as `console <name>`, as console programs do. The menu lists the ones on the disk: today `netbench` (moved from Network) and `memtest`.
- **Host test.** `tests/wm_host.rs` checks the category's entries and their `console` commands.
- **QEMU.** The `wm` suite's walk of every menu entry (000-APP-0056) starts the two in `console` windows.
- **Docs.** `docs/tools` (EN, RU) name the category.
- **Still to do:** the lines for `check`, `bench` and `kbench` in `help` and `docs/tools`, once they are in `main`.

## Related

[requests-APP.md](requests-APP.md) (where it was recorded).
