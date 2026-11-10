# 000-APP-0077 — The microphone's sensitivity: `mic` in the shell and on `wm`'s Sound page

**Type:** tools (`shell`, `wm`, `hear`) · **Owner:** tools track (`APP`) · **Priority:** P1 (the maintainer, 2026-10-30: "the microphone in `listen` is very quiet on playback; the sound's and the microphone's levels are needed") · **Status:** in progress (done in QEMU on x86; the MacBook Pro's check left, 2026-11-10) · **Blocked by:** — (`audio.wit` 1.2's levels, 000-DRV-0057, and the shell's badged client, 000-KRN-0118, are in `main`) · **Main task:** — · **Roadmap:** track G · **Constitution:** MC-3.11, MC-11.4, MC-11.5

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

## Progress (2026-11-10): done in QEMU

- **The shell's `mic`:**
  - `mic`, `mic <0–100>`, `mic boost on` and `mic boost off`, beside `volume` (000-APP-0076), in the same files;
  - the line names the gain, its dB and the boost, as `MIC 60 % (+13.50 DB OF 16 SETTINGS), BOOST OFF (+20.00 DB)`, or `NO BOOST`;
  - a boost the device does not have is refused.
- **Kept** in `data/sound.conf` (`gain`, `boost`) and set again at the shell's start.
- **`wm`'s Sound page:**
  - a slider for the gain and a box for the boost;
  - a live meter while the page shows: each half second's peak and RMS and the noise floor in dBFS, as `hear` measures them, through `wm`'s own audio client (`wm/src/meter.rs`);
  - under it, `The microphone records only while this page is open.` (MC-11.4). The capture stops when the page goes, and when `wm` ends;
  - another program's recording is waited for, and no device is named.
- **`hear`** names the gain it hears at, read through its plain client: `GAIN 60 % (+13.50 DB), BOOST OFF (+20.00 DB)`.
- **Authority:** only the shell's client may change the gain. A program that records, `wm` and `hear` among them, reads the levels and is refused setting them.
- **Docs:** `docs/tools` and `docs/voice` (EN, RU).
- **Tests** (QEMU x86, 4 CPUs, TCG, this track's container, 2026-11-10):
  - **host:** `tests/shell_host.rs` (the microphone's lines in `data/sound.conf`, a damaged file) and `tests/wm_host.rs` (the page with its meter);
  - **the `audio` suite, AC'97:**
    - `mic 60` reads back as 60 % (+13.50 dB of 16 settings), the boost on and off, `mic 120` and `mic 101` refused;
    - the Sound page's → set 65 %, which the shell read back as 67 %, the nearest setting;
    - the drivers track's checks of the gain at the device, and a plain client refused, in the same run.
  - **After a reboot,** on AC'97 and on HD Audio: `mic 80` set again from the file, read as 80 %.
  - **The meter,** on an AC'97 machine with QEMU's silent input:
    - it read -90 dBFS, digital silence, on the page and in the log;
    - `audio_gw` started the capture for `wm` and stopped it when Settings closed;
    - `hear` then recorded, so the microphone was free.
  - **The `wm` suite,** with no device: `No microphone to read.`
  - **The `listen` suite:** after `mic 60`, `hear 1`'s report names the gain.
- **aarch64:** the image builds with these programs, and its `normal` and `shell` suites pass on QEMU (4 CPUs, TCG). QEMU's aarch64 machine has no sound device, so the levels are not checked there.
- **Left:** the MacBook Pro: speech at arm's length reaching `hear`'s threshold at a gain the page sets, and `hear`'s level rising with `mic`.

## Related

[000-APP-0076](000-APP-0076-output-volume.md) (the volume), [000-APP-0053](000-APP-0053-hear-shows-what-it-heard.md) (`hear`'s levels), [000-APP-0048](../issues-done/000-APP-0048-wm-settings.done) (`wm`'s Settings), [551-DRV-0010](551-DRV-0010-hda-controller-and-codecs.md) (HDA capture), [551](551-sound-on-pcs.md) (sound on PCs).
