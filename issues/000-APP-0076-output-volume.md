# 000-APP-0076 — The output volume: `volume` in the shell and on `wm`'s Sound page

**Type:** tools (`shell`, `wm`) · **Owner:** tools track (`APP`) · **Priority:** P1 (the maintainer, 2026-10-30: "the microphone in `listen` is very quiet on playback; the sound's and the microphone's levels are needed") · **Status:** in progress (done in QEMU on x86; the MacBook Pro's check left, 2026-11-10) · **Blocked by:** — (`audio.wit` 1.2's levels, 000-DRV-0057, and the shell's badged client, 000-KRN-0118, are in `main`) · **Main task:** — · **Roadmap:** track G · **Constitution:** MC-3.11, MC-11.5

**Recorded by:** the tools track (APP), 2026-10-14, at the maintainer's request: "the output volume and the microphone's sensitivity need settings". The microphone's part is [000-APP-0077](000-APP-0077-microphone-sensitivity.md).

## Problem

Nothing sets how loud the sound is.

- `idl/audio.wit` 1.1 plays, records and makes tones, but has no volume:
  - `audio_gw` leaves AC'97's master volume at 0 dB and the PCM output at its default;
  - on HDA it unmutes the output amplifiers at 0 dB.
- A program can only scale its own samples. `tts`, `say` and `beep` play at the level they make.
- `wm`'s Settings has a Sound page that says so ([000-APP-0048](../issues-done/000-APP-0048-wm-settings.done)).

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

## Progress (2026-11-10): done in QEMU

- **The shell's `volume`** (`shell/src/sound.rs`, pure, and `shell/src/soundctl.rs`): `volume`, `volume <0–100>`, `volume mute` and `volume unmute`, through the shell's client of `audio_gw`, which `init` badges `BADGE_LEVELS`.
  - The line names the per cent, its dB and the device's settings, as `VOLUME 51 % (-46.50 DB OF 64 SETTINGS), NOT MUTED`, and says when the gateway scales because the device has no amplifier.
  - A level is the nearest setting the device has, so `volume 50` reads back as 51 % of AC'97's 64.
- **Kept across boots:** each change is written to `data/sound.conf` (`; KEPT IN data/sound.conf`, or `; UNTIL THE REBOOT: …` when it cannot be), and the shell sets the file's levels again at its start and logs them.
- **`wm`'s Sound page:**
  - it reads the levels through the shell's `volume` and `mic` every 2 s (`shell.wit`'s `run`; both are in the shell's list of commands taken at once);
  - a slider for the volume (← → by 5 %, a click) and a box for mute;
  - after a change of the volume, a test tone (880 Hz, 150 ms) through `wm`'s own client, which only plays;
  - without a device, the shell's error.
- **Authority:** only the shell's client may change the level; `audio_gw` refuses the others `rights` (000-DRV-0057).
- **Docs:** `docs/tools/README.md` and `README_RU.md` (`volume`, and the Sound page).
- **Tests** (QEMU x86, 4 CPUs, TCG, this track's container, 2026-11-10; the audio, wm, shell and net suites together, then the audio, wm, listen and tts suites):
  - **host:** `tests/shell_host.rs` (the words, the lines, `data/sound.conf` read, written, damaged) and `tests/wm_host.rs` (the page's rows, steps, clicks and messages);
  - **the `audio` suite, AC'97:**
    - `volume 50` reads back as 51 %, `mute` and `unmute`, `volume 101` and `volume loud` refused, the file's lines;
    - the drivers track's checks of the levels at the device, and a plain client refused setting them, in the same run;
    - the Sound page's ← set 46 % through the shell, `volume` read it, and its tone was found in the captured sound at 880 Hz for 150 ms.
  - **After a reboot,** on AC'97 and on HD Audio (QEMU's `hda-duplex`): `volume 50` and mute set again from `data/sound.conf`, `volume` reading 51 % and 50 %.
  - **The `wm` suite's machine has no sound device:** the page shows `NO AUDIO DEVICE`. The slider's check runs in the `audio` suite, whose machine has one.
- **No evidence of the level itself:** in QEMU's captured sound, the tone at 46 % was 3.5 dB below `beep` at 100 %. The `wav` backend does not follow AC'97's master attenuation, so this measures nothing about the volume at the device (MC-12.1).
- **aarch64:** the image builds with these programs, and its `normal` and `shell` suites pass on QEMU (4 CPUs, TCG). QEMU's aarch64 machine has no sound device, so the levels are not checked there.
- **Left:** the MacBook Pro: `volume 30` quieter than `volume 100` by ear, and `volume mute` silent.

## Related

[000-APP-0077](000-APP-0077-microphone-sensitivity.md) (the microphone), [000-APP-0048](../issues-done/000-APP-0048-wm-settings.done) (`wm`'s Settings), [551-DRV-0010](551-DRV-0010-hda-controller-and-codecs.md) (HDA in `audio_gw`), [211-APP-0044](../issues-done/211-APP-0044-console-joined-to-the-shell.done) (the shell's commands for `wm`).
