# APP ready: nnd, wm's keyboard page, the NET graph and the camera's fit check (replaces the request for 07d2ec2d)

**Ready:** `claude/wizardly-franklin-kec1a9` 4acb8b7c15b2504b058392c4bb1e1e819622ee4a

This replaces `LANDING/2026-10-29T1924Z-APP-ready-nnd-wms-keyboard-page-and-net-graph-replaces-d4f6b459.md` (07d2ec2d), not answered yet. It adds two commits to that request: a9c3e03f, a check in the `wm` suite, and 4acb8b7c, text only.

**What it carries, beyond main** (main d48010ed is in it):

- **262-APP-0112 (advances):** `idl/nn.wit` 1.0, `nnpkg` and the `nnd` program. `nnd` is a console program when run by hand (76f64ed2), the fix for round dbe7648e's `Other>nnd`.
- **000-APP-0048 (advances), 0e53e940:** `shell.wit` 1.10 `keymap` and `set-keymap`, and the Keyboard page of `wm`'s Settings.
- **000-APP-0049 (advances), 07d2ec2d:** the graph of the last minute's frames under the NET line on `wm`'s background.
- **211-APP-0046 (advances), a9c3e03f:** the `wm` suite's `camera_fit`. On the left half the synthetic camera's picture is scaled to fit; Alt+0 gives the frame back to 320 × 240, where the picture is the test pattern pixel for pixel. No program code changed for it.

**Tested** on this container (x86, QEMU TCG, 4 CPUs):

- On a9c3e03f's tree (4acb8b7c adds only text): the `wm` suite, all passed, no SKIP, among them `camera_fit` (frame 38 pixel for pixel), the menu walk (59 entries) and the keyboard page.
- On 07d2ec2d's tree: the `net` suite passed, and so did host test `wm_host` (46).
- On d4f6b459's tree: the `shell` and `tools` suites, and host tests `shell`, `idl`, `idl_fuzz` and `keys`, passed.
- On 10d7b849: the `services` and `link` suites passed, and so did `scripts/host_tests.sh` whole.
