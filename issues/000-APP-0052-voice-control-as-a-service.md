# 000-APP-0052 — Voice control as a service one can turn on

**Type:** tools · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open · **Blocked by:** `svc enable` (173) for the boot part; the microphone on the MacBook Pro (551-DRV-0010) for the hardware check · **Main task:** — · **Roadmap:** track G · **Constitution:** MC-3.11, MC-11.11

Numbered on 2026-10-10 from `requests-APP.md`, where the kernel track recorded it.

**Recorded by:** the kernel track (KRN), 2026-10-10, at the maintainer's request.

## Problem

The maintainer asked for "a service that can be switched on for audio commands, so that the system can be run by voice".

Today voice control lives in one shell:

- `voice on` starts the `voice` program for that shell's console only, and it stops with it;
- F12 is push-to-talk, and only while that console has the keyboard;
- nothing turns it on at boot, and `wm` and its windows have no part in it.

## Plan

- **A boot service**, off by default, that `svc enable voice` (173) turns on for the next boots and `svc start voice` for this one. It holds the microphone only while it listens.
- **How it listens.** It listens on push-to-talk from whichever program has the keyboard (the shell's F12, a key in `wm`), or continuously on a wake phrase if the maintainer chooses that later.
- **What it does.** It hands what it recognized to the shell's command endpoint (`SLOT_SHELL`, 211-KRN-0058), so the same confirmations hold: it asks before stopping a service or rebooting. In `wm` it can also open programs from the menu by name.
- **What it shows.** A mark that voice control is on and when it is listening, on the shell's line and in `wm`'s bar; every phrase heard and what was done with it goes to the system log.
- **Least authority (Art. 11.11).** It holds the audio, tts and read-only file clients and the shell's endpoint, nothing else.

## Acceptance criteria

- `svc enable voice` and a reboot leave voice control on, without a shell command.
- In QEMU, a WAV file standing in for the microphone opens a program and asks before a reboot.
- On the MacBook Pro, the same by the microphone once capture works there (551-DRV-0010).

## Related

[requests-APP.md](requests-APP.md) (where it was recorded).
