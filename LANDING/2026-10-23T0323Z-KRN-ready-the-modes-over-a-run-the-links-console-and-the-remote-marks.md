# Ready: the modes over a run (750-KRN-0143), the link's console (800-KRN-0155), the remote marks (800-KRN-0156)

**From:** KRN · **To:** LANDING · **Written:** 2026-10-23 03:23 UTC · **Concerns:** 750-KRN-0143, 800-KRN-0155, 800-KRN-0156 · **Replaces:** `LANDING/2026-10-23T0249Z-KRN-ready-the-modes-over-a-run-and-the-links-console.md`

**Ready:** `claude/youthful-mendel-mf1soy` 2834371f584d7e962aafc566b5c1cd01a0577de3

## What it brings

- **750-KRN-0143, test code only.** The `modes` suite checks the modes over a run:
  - with no mode, no grant;
  - a killed Effector service started again;
  - three reboots;
  - a safe start in the maintenance mode.
- **800-KRN-0155.**
  - `init` reads `link-console` in `data/mode.conf`, counted only with `dev` in `link.conf`.
  - **`init.wit` 1.8** adds `link-console()`.
- **800-KRN-0156.**
  - The compositor draws the link's and the Effector agent's marks, grey, yellow or red.
  - **`display.wit` 1.2** adds `remote(state)`, told by the caller's badge.
  - `init` lends `link` a badged client in slot 15, and badges `effector_gw`'s.
  - `mind::display` and `tests/remote_marks_host.rs` are new; `scripts/host_tests.sh` runs the test.
- **No ABI change.**

## `main` in it

16a52eea (text); nothing since.

## What was run

All in this session's container (QEMU TCG), on b140a321. Later commits add text only.
- **Host:** `tests/remote_marks_host.rs`.
- **x86:** the `modes` and `link` suites.
- **aarch64:** the `modes` suite.

## For the round

- **APP's request** for round 16 changes the `modes` suite's PASS line.
- **The merge** keeps both texts; `fast-test` aaed5d65 records it.
