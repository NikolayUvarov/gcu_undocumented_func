# 750-APP-0109 — The shell's `run` takes an installed application's path

**Type:** tools (`shell`) · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [750](750-effector-agent.md) · **Roadmap:** track G · **Constitution:** MC-9.1, MC-12.4

**Recorded by:** the tools track, 2026-10-28, from the kernel track's request in [requests-APP.md](requests-APP.md) ("`run` of an installed application's path", for [750-KRN-0169](../issues-done/750-KRN-0169-the-loader-checks-an-installed-application.done)).

## Problem

- **What the loader takes.** Since 750-KRN-0169 the loader runs a program under `data/apps/` only as its signed package lists it, in the version `data/apps/<name>/current` names. Its path is `data/apps/<name>/<file>` or `data/apps/<name>/<version>/<file>`, up to 63 bytes, as `loader.wit` takes (`string<64>`).
- **What the shell refuses.** The shell's `run` takes a program name of at most `NAME_MAX` (16) bytes and says `PROGRAM NAME TOO LONG` past it, so `run data/apps/memtest/memtest.elf` is refused before the loader sees it.
- **Why it matters.** Installed applications cannot be started from the shell by their real paths; the `modes` suite's check uses one-letter names to fit.

## Plan

- A name with `/` in it goes to the loader as a path of up to 63 bytes; a longer one is `PROGRAM PATH TOO LONG (63 BYTES AT MOST)`.
- A bare name keeps 16 bytes (`PROGRAM NAME TOO LONG`): it is a program's name, as `ps` shows it.
- The loader stays the one that checks the package; the shell adds no check of its own.

## Acceptance criteria

- **The `modes` suite's `installed_apps_check`** (x86, aarch64): a package named `memtest` runs by `data/apps/memtest/m` and `data/apps/memtest/1/m`, paths past 16 bytes; a 64-byte path and a 17-byte name are refused by the shell.
- **Docs:** `docs/tools/README.md` and `README_RU.md`; the shell's `help`.

## Related

[750-KRN-0169](../issues-done/750-KRN-0169-the-loader-checks-an-installed-application.done) (the loader's check), [750-EFF-0008](750-EFF-0008-application-packages.md) (the install by the gateway).
