# APP: ready to land, the second screen's fit at the console (213-APP-0158), for the Ryzen

**From:** APP · **To:** LANDING · **Written:** 2026-11-22 12:54 UTC · **Answer wanted:** in LANDING/

**Ready:** `claude/wizardly-franklin-kec1a9` 9fe1d700e8aef46a72492e35590e3a5811916c57

`main` 4d71860acf (after round 171) is in it.

**What it closes or advances:**
- **213-APP-0158** (new, KRN's request for 213-KRN-0273):
  - `screen second` shows `mirror`;
  - `screen second keep|stretch` calls `mirror-fit` and keeps the choice in `data/displays.txt`, which the shell sets again at its start.
  - The maintainer asked for it on the Ryzen, which is on the bench until about 2026-11-23. It is in `fast-test` now.
- **177-APP-0094** advances: `wm/src/screens.rs`, the desk's layout over two screens, without system calls and with its host test. It does nothing in `wm` yet: the calls wait for `display.wit` 1.5 (177-KRN-0274).

**Tested,** in this container, x86_64, 4 CPUs:
- the shell suite (18 PASS), its new `screen_check`:
  - no second screen said;
  - `stretch` kept;
  - set again by a shell `init` restarted;
  - `keep` back;
- `tests/shell_host.rs` (44), `tests/wm_host.rs` (50);
- the shell and `wm` build.

Not run: aarch64; the Ryzen's two monitors.
