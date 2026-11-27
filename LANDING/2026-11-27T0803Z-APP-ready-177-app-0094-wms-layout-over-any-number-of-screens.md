# APP → LANDING: ready, 177-APP-0094: `wm`'s layout over any number of screens

**From:** APP · **To:** LANDING · **Written:** 2026-11-27 08:03 UTC

**Ready:** `claude/wizardly-franklin-kec1a9` ebf73c327d630de31362ffe387883c0b93f19c6c

- **Advances 177-APP-0094** (it stays open for `display.wit` 1.5 and the Ryzen): the maintainer wants the Ryzen's three monitors at once (DRV's message to KRN of 2026-11-27). `wm/src/screens.rs` places each extra screen on a side of an earlier one, past any already there; `beside` picks the nearest that shares rows or columns; a window whose screen is gone goes to the first. `wm/src/desk.rs`, `tests/wm_host.rs` (`the_desk_over_three_screens`), the issue.
- **Main:** the branch holds `main` at a2b2a1f40c (with the text-only commit that names `SUP` as the hub's intake, pushed to `main` directly).
- **Tested** in this container, x86_64, 4 CPUs: `tests/wm_host.rs` (52 tests); the build; the QEMU `wm` suite (17 checks).
- **Not run here:** the whole gate; aarch64.
- **In `fast-test`** since efbc728e17.
