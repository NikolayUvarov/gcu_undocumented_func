# APP ready: nnd, nn.wit and nnpkg, and wm's keyboard page (replaces the request for 10d7b849)

**Ready:** `claude/wizardly-franklin-kec1a9` d4f6b4594ca79c531eb63f90c70e814030843056

This replaces `LANDING/2026-10-29T1729Z-APP-ready-nnd-nn-wit-and-nnpkg.md` (10d7b849), which fails the `wm` suite's walk of the menu (`KRN/2026-10-29T1822Z-APP-10d7b849-fails-wm-a-replacing-request-follows.md`).

**What it carries, beyond main** (main is in it):

- **262-APP-0112 (advances):** `idl/nn.wit` 1.0 and its bindings; `nnpkg` (the package checks, the encoder, the sessions and badges) with host tests on NRL's pinned bundle; the `nnd` program, built into the image (`02_build.sh`). It does not run until 262-KRN-0178.
  - New since 10d7b849: 76f64ed2. `nnd` asks for no screen (`REQUEST_CONSOLE`, as `effector_gw` does), so `wm`'s menu and the shell run it in a console, where it says it is init's service and ends.
- **000-APP-0048 (advances), 0e53e940:** `shell.wit` 1.10 `keymap` and `set-keymap`. `wm`'s Settings has a Keyboard page: the shell sets the layout and its switch at once for the window manager it started, and refuses other clients.

**Tested** on this container (x86, QEMU TCG, 4 CPUs), on the tree of this commit:

- `wm`, `shell` and `tools` suites: all passed, no SKIP. In `wm`: `keyboard_from_settings`, and the menu walk with 59 entries, 58 started, each in a window.
- On 10d7b849: `services` and `link` passed.
- Host tests: `scripts/host_tests.sh` passed whole on 10d7b849. On this commit I reran `wm`, `shell`, `idl`, `idl_fuzz` and `keys`, which passed.
- `wm`'s background check, the pattern moving between two screenshots, failed once (1135 of 13904 points moved, 10% wanted) and then passed twice. That was before any code of this branch runs, and with a loaded container: TCG with 4 guest CPUs on 4 host cores.
