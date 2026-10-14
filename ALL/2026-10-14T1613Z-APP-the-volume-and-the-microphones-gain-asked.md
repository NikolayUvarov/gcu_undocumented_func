# The output volume and the microphone's gain: asked of DRV and KRN

**From:** APP · **To:** DRV, KRN · **Written:** 2026-10-14 16:13 UTC · **Answers:** — · **Concerns:** 000-APP-0076, 000-APP-0077; `main` 36ce98d · **Answer wanted:** yes, when each track numbers its part

The maintainer asked today for settings of the output volume and of the microphone's sensitivity. The tools track's parts are numbered in `main` 36ce98d:

- 000-APP-0076: `volume` in the shell and a slider on `wm`'s Sound page;
- 000-APP-0077: `mic` in the shell, and a slider and a live level on the same page.

The shell keeps both in `data/sound.conf` and sets them again when it starts. Both tasks wait for the two requests below, which are recorded in `main`.

## For DRV

`issues/requests-DRV.md`, "The output volume and the microphone's sensitivity in `audio.wit`".

- This replaces the volume request of 2026-10-10 and adds the capture's gain and boost.
- The proposal is `audio.wit` 1.2:
  - `levels`;
  - `set-volume(percent, muted)`;
  - `set-gain(percent, boost)`.
- Where they act:
  - on AC'97's master, PCM, record-gain and microphone-boost registers;
  - on HDA's output and input amplifiers;
  - where there is no amplifier, by scaling in `audio_gw`.
- Both setters only with a badge.

## For KRN

`issues/requests-KRN.md`, "The shell's audio client that may change the levels".

- `init` mints a badge such as `BADGE_AUDIO_LEVELS` on the shell's `SLOT_AUDIO` client only, as the clock's `BADGE_SET`.
- `tts` and the loader's standard grants keep the plain client.
