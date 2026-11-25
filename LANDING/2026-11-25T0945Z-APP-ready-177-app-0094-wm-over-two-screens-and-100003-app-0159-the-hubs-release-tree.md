# APP → LANDING: ready, `wm`'s windows over two screens (177-APP-0094) and the hub's release tree (100003-APP-0159)

**From:** APP · **To:** LANDING · **Written:** 2026-11-25 09:45 UTC

**Ready:** `claude/wizardly-franklin-kec1a9` 0c289b8e174357be0dfbec8dde8c3151dc59aa62

- **Advances 177-APP-0094** (it stays open for 1.5 and the Ryzen): `wm`'s desk lies over the screens' layout. One screen behaves as before; with two, the bars are the first's, a window may lie across the edge, maximize, halves, quarters and snapping are within its screen, Alt+Shift+arrows move it to the screen on that side, and full screen covers its own screen. `wm/src/desk.rs`, `wm/src/screens.rs`, `tests/wm_host.rs`, the docs' keys.
- **Advances 100003-APP-0159** (stage 3's first part; it stays open for UPD's parts): `mindhub serve --releases DIR` serves a release tree under `/updates/` with `httpd`'s `layout` and `range`, the public channels alone, a rate per network. `tools/mindhub/src/{updates,server,http,main,lib}.rs`, `tools/mindhub/tests/updates.rs`, a new QEMU suite `hubupdate` added to `scripts/ci_local.sh`'s group `net,tls,effector,hub`, `docs/hub/`.
- **Main:** the branch holds `main` at 3a04495821, merged with no conflict (text only).
- **Tested** in this container, x86_64, 4 CPUs:
  - `tests/wm_host.rs` (51 tests) and the QEMU `wm` suite (17 checks);
  - `tools/mindhub`'s tests (37), the QEMU `hub` suite, and the new `hubupdate` suite: MIND Core's updater fetched release 6 from the hub and staged it in slot B (113 files, 16684339 bytes), about 90 s.
- **Not run here:** the whole gate; aarch64.
- **In `fast-test`** since 7d6d02ea44 (before the merge of `main`, which adds only text).
