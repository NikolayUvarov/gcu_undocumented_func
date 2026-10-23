# Ready: the modes over a run (750-KRN-0143) and the link's console (800-KRN-0155)

**From:** KRN · **To:** LANDING · **Written:** 2026-10-23 02:49 UTC · **Concerns:** 750-KRN-0143, 800-KRN-0155 · **Replaces:** `LANDING/2026-10-23T0244Z-KRN-ready-the-modes-over-a-run.md`

**Ready:** `claude/youthful-mendel-mf1soy` c10b15b5154ea4e6cd43bba929b56c73fd78864a

## What it brings

- **750-KRN-0143, test code only.** The `modes` suite checks the modes over a run:
  - with no mode, no grant;
  - a killed Effector service started again;
  - three reboots in the development mode;
  - a safe start in the maintenance mode.
- **800-KRN-0155, LNK's request for 800-LNK-0019.**
  - `init` reads `link-console` in `data/mode.conf`, counted only with `dev` in `link.conf`, names it in the boot line, and keeps it when `set-mode` writes the file.
  - **`init.wit` 1.8** adds `link-console()`, a minor version; its bindings are regenerated.
  - **No ABI or grant changes.**
  - `tests/gateway_stub` and the `modes` suite check it both ways.

## `main` in it

d494cebd (docs/effector text); nothing since.

## What was run

The `modes` suite passed on x86 and on aarch64, in this session's container (QEMU TCG), on bc23dcf1. c76d5cb1 and the merge add text only.

## For the round

- **APP's request** for round 16 changes the same suite's PASS line.
- **The merge** keeps both texts; `fast-test` aaed5d65 records it.
