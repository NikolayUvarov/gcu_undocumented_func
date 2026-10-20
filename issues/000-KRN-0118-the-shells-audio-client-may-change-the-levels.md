# 000-KRN-0118 — The shell's audio client may change the levels: `BADGE_AUDIO_LEVELS`

**Type:** `init`, `libmind` · **Owner:** kernel session · **Priority:** P3 · **Status:** open · **Blocked by:** the drivers track's `audio.wit` 1.2 (the request "The output volume and the microphone's sensitivity in `audio.wit`" in [requests-DRV.md](requests-DRV.md)) · **Main task:** — · **Constitution:** MC-3.4

## Problem

The maintainer asked for settings of the output volume and the microphone's sensitivity. The tools track (000-APP-0076, 000-APP-0077) wants them changed only by the shell:
- today `init` lends the shell a plain client of `audio_gw` in `SLOT_AUDIO`, as it lends `tts` one;
- programs get their own plain client from the loader's standard grants.

The tools track's request of 2026-10-14 is in [requests-KRN.md](requests-KRN.md).

## Plan

1. **`BADGE_AUDIO_LEVELS`** in `mind::audio`, as the clock's `BADGE_SET` (211-KRN-0051). The drivers track's `audio.wit` 1.2 serves the setters only to a client with it.
2. **`init` mints it** on the shell's client in `SLOT_AUDIO` only. `tts` and the loader's grants keep the plain client, so no program the shell starts gets it.

## Acceptance criteria

- [ ] `caps` shows the shell's audio client with the badge, and the client of a program the shell starts (`say`) without it.
- [ ] With `audio.wit` 1.2, `say` cannot change the volume and the shell can (the `audio` suite, with the drivers track's check).

## Related

The tools track's 000-APP-0076 and 000-APP-0077, [requests-DRV.md](requests-DRV.md), 211-KRN-0051 (the clock's setting badge).
