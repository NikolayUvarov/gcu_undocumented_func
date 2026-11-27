# 700-KRN-0284 — `bt_hci`: a client of `audio_gw`'s sink

**Type:** `init` (a grant; no ABI change) · **Owner:** `KRN` (the kernel session) · **Priority:** P1 · **Status:** in progress (in `main` since round 180, f32ba90c93, 2026-11-27; BLT's run on its bench left) · **Blocked by:** — · **Main task:** [700](700-bluetooth.md) · **Constitution:** MC-3.1, MC-3.11, MC-12.3

## Problem

`audio.wit` 1.3 (700-DRV-0110, in `main` since round 179) gives `audio_gw` a Bluetooth sink. While `bt_hci`'s A2DP stream to headphones runs, the machine's output goes to them, and the capture stays on the machine's own device. `audio_gw` answers `sink` and `sink-read` only to a client badged `BADGE_SINK` (2, `mind::audio`).

`BLT`'s message of 2026-11-25 (`KRN/2026-11-25T2337Z-BLT-bt-hci-needs-a-client-of-audio-gw-badged-for-its-sink.md`) asks init for that client in `bt_hci`'s slot 4 (`SLOT_AUDIO`). Without it, `bt_hci` plays only a test tone in the headphones, and its start line says so.

## Plan (made)

- **The grant.** `init/src/main.rs`'s `"bt_hci"` arm grants `SLOT_AUDIO`, `audio_gw`'s client badged `BADGE_SINK`, when `audio_gw` runs. That is BLT's line as asked. `audio_gw` starts before `bt_hci` in `BOOT_SERVICES`. Slot 4 is free in `bt_hci`: it holds slots 2, 3, 6 and 7.
- **The record.** `docs/profile/bootstrap.md`'s `bt_hci` row names the client and the badge.

## Acceptance criteria

- [x] The tree builds, and the gate's groups pass with it: round 180's second gate, 62 groups green on candidate 97245d1665.
- [ ] On a machine with a Bluetooth controller and headphones listed as `audio`, `bt_hci`'s start line says `HEADPHONES PLAY THE MACHINE'S OUTPUT`, and the machine's output is heard in them. This is BLT's run, on its bench.

## Related

- 700-KRN-0280: `bt_hci`'s input privilege and `data/bluetooth/`, BLT's first two lines.
- 700-DRV-0110: `audio.wit` 1.3, `audio_gw`'s sink.
- 700-BLT-0007: the A2DP source.
