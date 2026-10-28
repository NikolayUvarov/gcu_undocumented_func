# Ready: horizontal scrolling from a trackpad, through `wm` to `view`

**From:** APP · **To:** LANDING · **Written:** 2026-10-28 11:55 UTC

**Ready:** `claude/wizardly-franklin-kec1a9` 67d8df19ec7d478c7537104cc8f0b5dfc29edf26

## What it closes or advances

- **Closes [211-APP-0090](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/claude/wizardly-franklin-kec1a9/issues-done/211-APP-0090-horizontal-scrolling.done)**, for 211-DRV-0018, on 211-KRN-0150's `pointer_across`:
  - `mind::input::Pointer` gains `across`;
  - `mind::window::pointer_at` takes it, through `pointer_absolute_scroll`;
  - `wm`'s `pointer_scroll` sends it where the wheel goes, to the window under the pointer without raising it, and `Action::Pointer` carries it;
  - `view` turns a step into ← or →.
- **Code:** `libmind/src/input.rs`, `libmind/src/window.rs`, `wm/src/desk.rs`, `wm/src/main.rs`, `view/src/main.rs`.
- **Tests:** `tests/wm_host.rs` and `tests/window_host.rs`.
- **Docs:** `docs/tools/README.md` and `README_RU.md`.
- **Not shown:**
  - no QEMU device sets the field (only `usb_hid`'s trackpad does), so no suite shows it end to end;
  - the MacBook Pro's swipe is the maintainer's run.
- The rest is `main` 8721ef86 and its text commits, merged in.

## What was tested, and where (on 67d8df19, `main` 6467887d merged in)

- **Host tests:** `tests/wm_host.rs` 44, `window_host.rs` 6, `viewer_host.rs` 5, `shell_host.rs` 19, all passed.
- **x86, in the tools track's container** (QEMU, TCG, 4 CPUs, OVMF): the build, clean; then passed:
  - `--suites windows,wm,tablet,usb,ehci`, 19 PASS lines;
  - `--suites shell,tools,keys`, 22.
- **aarch64, in the same container:** the build; then `--suites normal,shell,bench,usb,devicetree,efivar`, 27 PASS lines.
  - That is the group "aarch64: programs, shell and four CPUs" without `smp` and `busy`, whose test programs this container does not build for aarch64.
  - `wm` does not run on aarch64's virt.
