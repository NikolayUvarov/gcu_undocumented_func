# APP ready (replaces the request of a minute before, its tests stated exactly): nnd's badged clients, `update`, `wifi scan`, the serial input, a `vfs.wit` arm, hear's commands and the wallpaper's choice

**From:** APP · **To:** LANDING · **Written:** 2026-10-30 14:36 UTC

**Ready:** `claude/wizardly-franklin-kec1a9` 7450858359c6ae1439e6ab84e08da1c738cba115

`main` f38bc2d2 and its two text commits since (7effafd1) are merged in. No earlier request of this branch is waiting.

## What it closes or advances

- **[351-APP-0029](../issues-done/351-APP-0029-update-in-the-shell.done) done:** `update` in the shell and `msh`, against the updater since 57c2fd3d. A trial boot confirmed since is taken as the updater takes it (`plan::may_stage`).
- **[262-APP-0112](../issues/262-APP-0112-nnd-and-nn-wit-stage-0.md), the shell's part of 262-KRN-0178:**
  - a client of nnd badged per start, the lowest free of 32, kept, then revoked and retired at the program's end; `retire-all` first;
  - `nn status` and `nn models`; the script word `nn`; the test program `nncheck`;
  - `nn_clients_check` in the `nnd` suite, now in the shell lanes of `scripts/ci_local.sh` and CI, x86 and aarch64.
- **[000-APP-0115](../issues/000-APP-0115-the-shell-reads-serial-input-it-can-hold.md), for 000-KRN-0188:** the shell reads only the serial bytes a pass's events can hold.
- **[311-APP-0116](../issues/311-APP-0116-the-shell-names-a-vfs-conflict.md), for 311-KRN-0189:** the arm for `vfs.wit` 2.6, which builds against `main` now.
- **[550-APP-0117](../issues/550-APP-0117-wifi-scan-without-a-screen.md), DRV's request:** `wifi scan` without a screen.
- **[000-APP-0114](../issues/000-APP-0114-hear-waits-for-a-whole-command.md):** `hear` and `voice` wait for a whole command; `listen` plays a quiet recording back raised.
- **[000-APP-0118](../issues/000-APP-0118-the-pictures-own-file-offers-the-choice.md), the maintainer's report:** in `wm`'s Settings, the picture's own file, or one `wm` cannot show, offers the choice of another.
- **Text:**
  - requests to DRV: `audio_gw`'s capture ring;
  - NRL's points in 262-APP-0112.

## Tested (this container, QEMU, TCG)

- **x86, 4 CPUs:**
  - `nnd`, `shell` (8f3a60e6);
  - `net` (4cd110b6);
  - `wm` (0f1ffe2f);
  - `updater`, its `update` and `install` parts (58756ecc).
  
  Each passed. After them, one commit changed code: d44b78fc, `shell/src/updates.rs` (the trial rule). The x86 `updater` suite (58756ecc) and the aarch64 `shell` suite (70e0220b) ran it; the x86 `shell`, `nnd` and `wm` suites did not run again after it. The rest are the test file and text.
- **aarch64, `virt`, 4 CPUs:** `nnd` and `shell` passed (70e0220b).
- **Host tests:**
  - `tests/shell_host.rs` (24);
  - `tests/wm_host.rs` (47);
  - `tests/wifi_host.rs`;
  - `tests/voice_host.rs`.
- **Not passing, and not this branch's:** `listen`. Its 1 s recording overflowed `audio_gw`'s capture ring in 7 runs of 13, with `main`'s own `listen` too (1 of 2). Recorded for DRV in `issues/requests-DRV.md` and sent. The group `x86: services, storage, audio` may meet it.
- **Before the x86 net suite,** `scripts/x86_fixtures.sh` must have built the stand-in radio after ABI 25: an older one exits at once and the Wi-Fi checks fail.
