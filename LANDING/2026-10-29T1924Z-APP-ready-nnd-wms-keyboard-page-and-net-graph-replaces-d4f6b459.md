# APP ready: nnd, nn.wit and nnpkg, wm's keyboard page and the NET graph (replaces the request for d4f6b459)

**Ready:** `claude/wizardly-franklin-kec1a9` 07d2ec2dc59a4b007504f535b5c9064ccbcdf005

This replaces `LANDING/2026-10-29T1846Z-APP-ready-nnd-and-wms-keyboard-page-replaces-10d7b849.md` (d4f6b459), which was not answered yet. The only change is one more commit, 07d2ec2d. The request for 10d7b849 was returned in round dbe7648e for `Other>nnd`. 76f64ed2, in this request too, fixes that: `nnd` asks for no screen, so `wm`'s menu runs it in `console`.

**What it carries, beyond main** (main d48010ed is in it):

- **262-APP-0112 (advances):** `idl/nn.wit` 1.0 and its bindings; `nnpkg` with host tests on NRL's pinned bundle; the `nnd` program, built into the image, a console program when run by hand (76f64ed2). It does not run as a service until 262-KRN-0178.
- **000-APP-0048 (advances), 0e53e940:** `shell.wit` 1.10 `keymap` and `set-keymap`, and the Keyboard page of `wm`'s Settings.
- **000-APP-0049 (advances), 07d2ec2d:** under the NET line on `wm`'s background, a graph of the last minute's frames each way, with its scale named under it.

**Tested** on this container (x86, QEMU TCG, 4 CPUs):

- On 07d2ec2d's tree: the `wm` suite (the menu walk with 59 entries, 58 started, each in a window; the keyboard page) and the `net` suite (`wm`'s NET line with `show = net`), all passed, no SKIP. Host tests `wm_host` (46, among them the graph's) passed.
- On d4f6b459's tree: the `shell` and `tools` suites; host tests `shell`, `idl`, `idl_fuzz` and `keys`. All passed.
- On 10d7b849: `services` and `link` passed, and `scripts/host_tests.sh` passed whole.
