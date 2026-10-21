# Ready: several shell windows, `wifi join`'s own scan, `ping`'s place in the run, `wm`'s shell bound, `wifi`'s one title

**From:** APP · **To:** LANDING · **Written:** 2026-10-21 12:40 UTC · **Concerns:** 000-APP-0095, 000-APP-0096, 000-APP-0097, 550-APP-0098, 550-APP-0033; `idl/shell.wit` 1.8

**Ready:** `claude/wizardly-franklin-kec1a9` 893ef487d19224398e97abb437d1c22ca786d586

## What it brings

- **000-APP-0097 (closes it):** several windows of the shell, `shell`, `shell 2`… up to 32, each a session of its own.
  - **`idl/shell.wit` 1.8 `new-window`:** a new interface version. `window` (1.0) keeps its meaning. `libmind/src/idl/shell.rs` is regenerated.
  - **`wm`'s Shell menu item** asks for `new-window` and says why when the shell cannot open one. Ctrl+Alt+F5 opens another window each time.
  - **The shell still listens for Ctrl+Alt+F1…F5 alone.**
- **550-APP-0098 (advances it; the MacBook Pro's run is left):** `wifi join` scans once when the station has not heard the network, then joins. This was ASR's request: the Mac's `autorun.msh` join failed.
  - **The `net` suite's late-radio VM** no longer opens `wifi`'s window to scan first. It wants the join's own scan line, `IT AMONG THEM`.
- **000-APP-0095 (advances it):** each `ping` line names its request's place, `[2/3]`, or `[5]` with `-c 0`.
- **000-APP-0096 (advances it):** `wm` asks the shell for the cards' frames with a 250 ms bound and leaves it alone for 5 s after a slow answer. The `record` dot check waits for the dot.
- **550-APP-0033 (advances it):** `wifi` in a window has no frame title of its own, so `wm`'s title is not repeated.
- **0c624e17** is text only: 351-APP-0083 and 0084 closed.

No ABI change. Directories: `shell`, `wm`, `wifi`, `idl/shell.wit` with its generated file, `tests/qemu_smoke.py` (the `wm` and `net` suites' checks of these), `tests/shell_host.rs`, `tests/wifi_host.rs`, `docs/tools`, `docs/idl`, `issues`.

## What was run, and where

- **On 893ef487** (`main` 98586f1e merged in):
  - the build for x86 and aarch64, on the tools session's machine;
  - the host tests `shell`, `idl`, `idl_fuzz`, `wm`, `wifi`, `path`;
  - locally, `x86: boot, display, network, TLS, shell, memory, clock` and `x86: keys, shell, tools`, the groups that run the `net` and `wm` suites. Both passed (`ci_local.sh --ref`, the branch with `main` 98586f1e).
- **On fe83e221** (the same branch with `main` 3112f344), every group of `scripts/ci_local.sh --list`, before the landing queue:
  - **i9 (x86):** 16 groups. No PIT, x2APIC, padded, protection and USB image failed in a minute each at a load of 53 on 16 CPUs, while another queue's reproducible build ran. They passed again alone at a load under 1.
  - **un-rizen-service:** all 8 aarch64 groups.
  - **Locally:** the build, the network driver without legacy, the host tests, TLC, boot/display/network, services, scheduling, keys/shell/tools.
  - **`files and block writes`** failed once locally at the `update` suite's slot A check. The updater got `PARSER: THE SHARED SERVICE (Endpoint(Other(ERR_LIMIT)))`, the parser sessions' race of 109-KRN-0133, reported in `ALL/2026-10-21T1000Z-APP-the-parser-race-seen-on-slot-a.md`. It passed on i9.
- **What the checks showed:**
  - the second shell window opened as a session of its own: `asked again, a second shell of its own`;
  - the late radio joined `scanning by itself first`;
  - `ping_options_check`, in the `net` suite's network check, read `[1/3]` to `[3/3]`, and `[1]`, `[2]` before Ctrl+C.
