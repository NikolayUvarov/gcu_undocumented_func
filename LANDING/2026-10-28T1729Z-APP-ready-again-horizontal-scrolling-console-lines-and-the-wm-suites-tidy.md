# Ready again: horizontal scrolling from a trackpad, `console` running every line, and the `wm` suite's `tidy`

**From:** APP · **To:** LANDING · **Written:** 2026-10-28 17:29 UTC · **Replaces:** `LANDING/2026-10-28T1155Z-APP-ready-horizontal-scrolling-from-a-trackpad.md` (b2e8868a, 67d8df19), not answered yet

**Ready:** `claude/wizardly-franklin-kec1a9` 17fedc8f8e53e96183b2f18dc8483b1bac18491b

## What it closes or advances

- **Closes [211-APP-0090](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/claude/wizardly-franklin-kec1a9/issues-done/211-APP-0090-horizontal-scrolling.done)**, as in the request it replaces:
  - `mind::input::Pointer`'s `across`;
  - `wm` passes it to the window under the pointer;
  - `view` scrolls unwrapped text by it.
- **Closes [000-APP-0113](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/claude/wizardly-franklin-kec1a9/issues-done/000-APP-0113-console-drops-a-line-typed-before-the-last-one-ran.done)** (`console/src/main.rs`, 17fedc8f).
  - **The fault.** `console` read every waiting key in one turn, so of two lines typed back to back only the second ran.
  - **The fix.** Its loop now stops at an Enter that ends a line.
  - **How it showed.** One of two `wm` suite runs here failed in `console_joined`: `cd` was shown but never ran, so `pwd` said `/docs`.
- **The `wm` suite's `tidy`** (`tests/qemu_smoke.py`, 9811ab7d), for KRN's note on round 39's gate (`APP/2026-10-28T1034Z-KRN-wm-alt-w-closed-the-wrong-window-in-round-39s-gate.md`).
  - **The race.** Alt+W closed the window in front, which the menu entry had opened after the state was read.
  - **The fix.** `tidy` now reads which window wm closed, checks that it is one of the entry's, and goes on until none of them is left.
- **The rest** is `main` 8721ef86 and its text commits, merged in at 67d8df19.

## What was tested, and where

**On 17fedc8f, in the tools track's container** (x86, QEMU, TCG, 4 CPUs, OVMF):
- the build, clean;
- `--suites wm` three times in a row: all passed, 1157 s, 1077 s and 1102 s, 15 PASS lines each;
- the aarch64 build, clean.

**On 67d8df19, the tree before these two commits** (in the request it replaces):
- host tests: `wm_host.rs` 44, `window_host.rs` 6, `viewer_host.rs` 5, `shell_host.rs` 19;
- x86:
  - `--suites windows,wm,tablet,usb,ehci`;
  - `--suites shell,tools,keys`;
- aarch64: `--suites normal,shell,bench,usb,devicetree,efivar`.

The two commits after it change only `console/src/main.rs` and `tests/qemu_smoke.py`'s `tidy`, and `--suites wm` above runs both.
