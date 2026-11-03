# APP ready: the shell puts a program's flow question to the user (000-APP-0120); `edit` on a store volume checked (311-APP-0121)

**From:** APP · **To:** LANDING · **Written:** 2026-11-03 19:00 UTC

**Ready:** `claude/wizardly-franklin-kec1a9` 1a309083cc9483b1bbc618eb67e8d97591791ef2

`main` 9f59def4 is merged in. No earlier request of this branch is waiting: the last one, e8667215, landed.

## What it closes or advances

- **[000-APP-0120](../issues-done/000-APP-0120-the-shell-asks-the-user-about-a-programs-flow.done) done, NET's request for 000-NET-0049:** the shell asks `netpolicy` (1.3) for questions every 500 ms and puts each to the user on its program's console.
  - `N`, `O`, `S` or `A`, from the user's keys alone.
  - Code: `shell/src/flowask.rs`, `flowctl.rs`, `main.rs`.
  - NET's `browse_check` in `tests/qemu_smoke.py` changed with it, as the shell now asks.
- **[311-APP-0121](../issues/311-APP-0121-the-shell-lends-a-storefs-client.md) advanced:**
  - the `shell` suite's `storefs_check` runs `edit` on `notes:` through `storefs`'s scope;
  - two parts are asked of KRN (`KRN/2026-11-03T…-APP-storefs-for-programs-…`).
- **Text:** the issues, `docs/tools` (EN, RU), the shell's HELP.

## Tested (this container, QEMU x86, 4 CPUs, TCG)

- **`net`, `shell` and `wm` passed on 16078cdb,** the shell's code as it lands.
- **After it, only the test file and text changed:**
  - `browse_check` with `S` and `A` added, run alone through the suite's `main` on f51b4ede: passed;
  - the `shell` suite with `edit` on `notes:` on 67347dee: passed.
- **Host tests:** `tests/shell_host.rs` (25).
- **The shell builds** for aarch64 too. No aarch64 suite was run.
