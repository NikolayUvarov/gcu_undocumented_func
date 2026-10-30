# 000-APP-0077 — The microphone's sensitivity: `mic` in the shell and on `wm`'s Sound page

**Type:** tools (`shell`, `wm`, `hear`) · **Owner:** tools track (`APP`) · **Priority:** P1 (the maintainer, 2026-10-30: "the microphone in `listen` is very quiet on playback; the sound's and the microphone's levels are needed") · **Status:** open (planned; waits for DRV and KRN) · **Blocked by:** `DRV`: the levels in `audio.wit` ([requests-DRV.md](requests-DRV.md), "The output volume and the microphone's sensitivity in `audio.wit`"); `KRN`: the shell's right to change them ([requests-KRN.md](requests-KRN.md), "The shell's audio client that may change the levels") · **Main task:** — · **Roadmap:** track G · **Constitution:** MC-3.11, MC-11.4, MC-11.5

**Recorded by:** the tools track (APP), 2026-10-14, at the maintainer's request: "the output volume and the microphone's sensitivity need settings". The output's part is [000-APP-0076](000-APP-0076-output-volume.md).

## Problem

Nothing sets how sensitive the microphone is.

- **What `audio_gw` sets today.** It fixes the capture gain at start and never changes it:
  - AC'97: the microphone at 0 dB without its +20 dB boost, and the record gain at 0 dB;
  - HDA: the input amplifiers unmuted at 0 dB.
- **What follows.** At a fixed gain, a quiet microphone gives speech near the noise floor and a loud one clips; the user can do nothing about either.
- **What the programs can do.** `hear` shows the level, the noise floor and the peak in dBFS (000-APP-0053), but neither it nor `listen` and `voice` can raise the gain. Scaling the samples raises the noise as much as the speech.

## Plan

Once `audio.wit` has the capture's gain and the shell may change it:

- **The shell's `mic` command:**
  - `mic` shows the gain in per cent and in dB where the device says, and whether the boost is on;
  - `mic <0–100>` sets the gain, `mic boost on` and `mic boost off` switch the boost where the codec has one;
  - the help line, and `docs/tools` (EN, RU).
- **Kept across boots,** in `data/sound.conf` beside the volume (000-APP-0076), set again when the shell starts.
- **`wm`'s Sound page:**
  - a slider for the gain and a box for the boost;
  - a live meter of the level while the page is open: peak and RMS in dBFS, with the noise floor, as `hear` measures them (000-APP-0053);
  - the meter records only while the page shows it and says so on the page, since the microphone is a sensor of the user's surroundings (MC-11.4);
  - it asks the shell through its commands, as for the volume.
- **`hear`** names the gain it heard at (`GAIN 60% +12 DB`), so its reports can be compared.
- **Authority:** as for the volume, only the shell's client may change the gain, and `audio_gw` checks it (MC-3.11, MC-11.5). A program that records does not get the right to make the microphone more sensitive.
- **Without a gain** (no capture device, or an older `audio_gw`), the command and the page say so.

## Acceptance criteria

- **Host test:** the microphone's part of `data/sound.conf` (the gain, the boost, a damaged file).
- **The `audio` suite** (x86, AC'97 and HDA):
  - `mic 80` reads back as 80;
  - the drivers track's check of the gain at the device passes with it;
  - after a reboot, `mic` shows 80;
  - `mic 101` is refused.
- **The `wm` suite:** the Sound page sets the gain, and its meter shows a level and stops recording when the page closes (`audio_gw` frees the capture).
- **A program lent the audio client cannot change the gain:** it gets `rights`.
- **On the MacBook Pro:**
  - speech at arm's length reaches `hear`'s speech threshold at a gain the page sets;
  - `hear`'s level rises with `mic`.

## Related

[000-APP-0076](000-APP-0076-output-volume.md) (the volume), [000-APP-0053](000-APP-0053-hear-shows-what-it-heard.md) (`hear`'s levels), [000-APP-0048](000-APP-0048-wm-settings.md) (`wm`'s Settings), [551-DRV-0010](551-DRV-0010-hda-controller-and-codecs.md) (HDA capture), [551](551-sound-on-pcs.md) (sound on PCs).
