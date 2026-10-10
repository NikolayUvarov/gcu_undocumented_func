# 000-APP-0048 — `wm`: Settings, one entry point in the top bar

**Type:** tools (`wm`, `shell`) · **Owner:** tools track (`APP`) · **Priority:** P2 (the maintainer's request, 2026-10-10) · **Status:** in progress (the window and the background's page done; the system's pages wait for 211-APP-0044) · **Blocked by:** — (the system's pages: 211-APP-0044 for the shell's channel; the sound: a volume control in `audio.wit`) · **Main task:** — · **Roadmap:** track G · **Constitution:** MC-3.11, MC-11.5

The maintainer's request (2026-10-10): "Add a background switch to the top menu: we will put settings there, where the screen's parameters (network, sound and the rest that needs configuration) can be managed too, through one entry point."

## Problem

What can be configured is spread over shell commands (`keymap`, `date set`, `netpolicy`, `ip`, `voice`) and files; `wm` has no place for it, and the desktop background (000-APP-0047) needs a switch.

## Plan

- **The entry point:** a `Settings` item in `wm`'s top bar (and a key, Alt+S) opens a settings window of `wm`'s own: a list of pages on the left, the page on the right, keys and the mouse.
- **Page 1, Background** (with 000-APP-0047): the kind (none, abstract, image and its file) and what is shown over it (date, time, CPU, network); a change shows at once and is kept in `data/wm.conf`.
- **The system's pages** act through the shell, the one holder of the operator's authority (211-APP-0044's channel, `SLOT_SHELL`), and each change that touches the machine is confirmed:
  - **Keyboard:** the layout and the layout switch (`keymap`);
  - **Date and time:** `date set`;
  - **Network:** the addresses (`ip`), the flow grants and the policy's lines (`netgrants`, `netpolicy`);
  - **Sound:** the volume, once `audio.wit` has one (asked of the track that owns `audio_gw`);
  - **Screen:** what the firmware's mode allows (shown; changed when a way to change it exists).
- Until the shell's channel is in `main`, the system pages say where the setting is made today (the shell's command).
- **Docs:** `docs/tools` (EN, RU), `wm`'s help and top bar.

## Acceptance criteria

- **The `wm` suite:** the top bar's `Settings` (a click, and Alt+S) opens the settings window; the background page switches between none, abstract and an image, the desktop follows at once, and the next `wm` starts with the choice (read back from `data/wm.conf`).
- With 211-APP-0044: the keyboard page switches the layout through the shell; the network page lists the flow grants.
- **Host tests:** the settings window's layout and its keys.

## Progress (2026-10-10)

- **Done:** `wm/src/settings.rs` and `wm`: the top bar's last item `Alt+S settings` and Alt+S open the Settings window (pages on the left: Background, Keyboard, Date and time, Network, Sound, Screen). The Background page changes the picture (none, abstract, image and its typed file), what is shown over it and its place, with keys and clicks; each change is used at once and written to `data/wm.conf` (`[WM] SETTINGS … SAVED`), and the next `wm` starts with it. The system's pages say where each setting is made today.
- **Checked:** host tests (the bar item and Alt+S, the picture cycling, a toggle, the place, a typed file, the system pages' text, clicks); the `wm` suite (Alt+S, the picture to an image's fallback and to none, the desktop's `░` cells at once, the next `wm` reading `none`).
- **Added with [000-APP-0050](../issues-done/000-APP-0050-wm-background-pictures-and-pattern-settings.done):** rows for the pattern's kind, speed, contrast and complexity and the information's brightness; ← → step them, Space and Enter go round.
- **Left:** the system's pages acting through the shell (211-APP-0044's `run`, still to come; `SLOT_SHELL` is in `main`); the sound's volume (requests-DRV.md).

## Related

[000-APP-0047](../issues-done/000-APP-0047-wm-desktop-background.done), [211-APP-0044](211-APP-0044-console-joined-to-the-shell.md), [u008](../issues-done/u008-clickable-top-bar.done).
