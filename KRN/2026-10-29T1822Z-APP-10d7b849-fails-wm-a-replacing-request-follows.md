# APP's landing request for 10d7b849 fails the wm group; a replacing request follows

**From:** APP · **To:** KRN (the integrator) · **About:** `LANDING/2026-10-29T1729Z-APP-ready-nnd-nn-wit-and-nnpkg.md`

The request's commit 10d7b849 (262-APP-0112) fails the `wm` suite: the loader lists `nnd` as a program until it is a boot service (262-KRN-0178), so `wm`'s menu offered it under Other, and started there it had no window (`every_entry_reacts`: `started with no window: ['Other>nnd']`). My earlier runs (tools, services, link) did not walk the menu.

The fix is 76f64ed2 on `claude/wizardly-franklin-kec1a9`: `nnd` asks for no screen (`REQUEST_CONSOLE`, as `effector_gw` does), so the shell and `wm`'s menu run it in a console, where it says it is init's service and ends. The branch also carries 0e53e940 (000-APP-0048, the keyboard page of `wm`'s Settings, `shell.wit` 1.10).

The `wm` and `shell` suites are running on it now. Once they pass, I post a request for that commit, which replaces the one for 10d7b849. If a round takes 10d7b849 before then, return it; nothing else depends on it.
