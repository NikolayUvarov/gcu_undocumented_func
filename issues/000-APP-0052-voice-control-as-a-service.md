# 000-APP-0052 — Voice control as a service one can turn on

**Type:** tools · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** in progress (done in QEMU on x86, the boot part through the shell; the MacBook Pro's microphone left, 2026-11-12) · **Blocked by:** the microphone on the MacBook Pro (551-DRV-0010) for the hardware check · **Main task:** — · **Roadmap:** track G · **Constitution:** MC-3.11, MC-11.11

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

## Progress (2026-11-12)

- **The boot part, through the shell, not `init`.** Voice control needs the shell's own endpoint (`voice.wit`'s line from `voice` to the shell), which only the shell makes (211-KRN-0058), so `init` cannot start it as a boot service. So the plan's `svc enable voice` became the shell's own setting, as `data/sound.conf` is:
  - `voice enable [--wav FILE] [SECONDS]` keeps `voice on`'s words in `data/voice.conf`; `voice disable` removes it; words `voice on` would not take are refused (`shell/src/voiceconf.rs`, `voicectl.rs`);
  - at its start the shell reads the file once the boot volume answers, after the boot's passphrase where one is asked, and turns voice control on with those words, logging `[SHELL] VOICE CONTROL ON AT THE SHELL'S START, AS data/voice.conf SAYS`;
  - `voice` holds what it held: the line to the shell, audio, `tts` and read-only files.
  So the first criterion reads: `voice enable` and a reboot leave voice control on, without a shell command.
- **Tests:**
  - host: `tests/shell_host.rs` `voice_control_at_the_shells_start_is_kept_in_voice_conf` (36 tests pass);
  - QEMU x86, 4 CPUs, TCG, this track's container, 2026-11-12: the `listen` suite's voice control checks `voice enable`'s refusal and file and `voice disable`; `voice_at_start_check` boots with `data/voice.conf` naming a WAV of three phrases: the shell turned voice control on with no command, «открой файлы» opened `fm`, «перезагрузи систему» was asked about and «нет» cancelled it, the machine running on.
  - The `listen` suite passed with the first; the second passed alone after its first run read the voice line where the VM's own `ps` had taken it.
- **Docs:** `docs/voice/README.md` and `README_RU.md`; `help`.
- **Left:** the MacBook Pro's microphone (551-DRV-0010).

## Related

[requests-APP.md](requests-APP.md) (where it was recorded).
