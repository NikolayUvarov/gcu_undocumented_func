# 000-APP-0076 — The output volume: `volume` in the shell and on `wm`'s Sound page

**Type:** tools (`shell`, `wm`) · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open (planned) · **Blocked by:** `DRV`: the levels in `audio.wit` ([requests-DRV.md](requests-DRV.md), "The output volume and the microphone's sensitivity in `audio.wit`"); `KRN`: the shell's right to change them ([requests-KRN.md](requests-KRN.md), "The shell's audio client that may change the levels") · **Main task:** — · **Roadmap:** track G · **Constitution:** MC-3.11, MC-11.5

**Recorded by:** the tools track (APP), 2026-10-14, at the maintainer's request: "the output volume and the microphone's sensitivity need settings". The microphone's part is [000-APP-0077](000-APP-0077-microphone-sensitivity.md).

## Problem

Nothing sets how loud the sound is.

- `idl/audio.wit` 1.1 plays, records and makes tones, but has no volume:
  - `audio_gw` leaves AC'97's master volume at 0 dB and the PCM output at its default;
  - on HDA it unmutes the output amplifiers at 0 dB.
- A program can only scale its own samples. `tts`, `say` and `beep` play at the level they make.
- `wm`'s Settings has a Sound page that says so ([000-APP-0048](000-APP-0048-wm-settings.md)).

## Plan

Once `audio.wit` has the output's level and the shell may change it:

- **The shell's `volume` command:**
  - `volume` shows the output's level in per cent, in dB where the device says, and whether it is muted;
  - `volume <0–100>` sets it, `volume mute` and `volume unmute` mute and unmute it;
  - the help line, and `docs/tools` (EN, RU).
- **Kept across boots.** The shell writes the level to `data/sound.conf` and sets it again when it starts.
  - Without a data volume, the level lasts until the reboot, and the shell says so.
  - A level the file holds but the device cannot take is set to the nearest it can.
- **`wm`'s Sound page:**
  - a slider for the volume with keys and clicks, and a mute box;
  - a test tone at the new level;
  - it asks the shell through its commands (`shell.wit`'s `run`, 211-APP-0044); `volume` is taken at once: it changes no authority.
- **Authority** (MC-3.11):
  - only the shell holds the client that may change the level;
  - the clients programs get (`tts` from `init`, `say` and `beep` from the loader) only play;
  - `audio_gw` checks the right on its side (MC-11.5).
- **Without a level** (no device, or an older `audio_gw`), the command and the page say so and nothing breaks.

## Acceptance criteria

- **Host test:** `data/sound.conf` is read and written (the level, mute, and a damaged or missing file).
- **The `audio` suite** (x86, AC'97 and HDA):
  - `volume 50` reads back as 50;
  - the drivers track's check of the level at the device passes with it;
  - after a reboot, `volume` shows 50;
  - `volume 101` and `volume loud` are refused.
- **The `wm` suite:** the Sound page's slider sets the level, and `volume` in the shell reads it.
- **A program lent the audio client cannot change the level:** it gets `rights`.
- **On the MacBook Pro:** `volume 30` is quieter than `volume 100`, by ear, and `volume mute` is silent.

## Related

[000-APP-0077](000-APP-0077-microphone-sensitivity.md) (the microphone), [000-APP-0048](000-APP-0048-wm-settings.md) (`wm`'s Settings), [551-DRV-0010](551-DRV-0010-hda-controller-and-codecs.md) (HDA in `audio_gw`), [211-APP-0044](../issues-done/211-APP-0044-console-joined-to-the-shell.done) (the shell's commands for `wm`).
