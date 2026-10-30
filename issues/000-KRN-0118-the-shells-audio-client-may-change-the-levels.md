# 000-KRN-0118 — The shell's audio client may change the levels: `BADGE_AUDIO_LEVELS`

**Type:** `init`, `libmind` · **Owner:** kernel session · **Priority:** P1 (the maintainer's report of 2026-10-30: the microphone is too quiet; APP raised 000-APP-0076 and 0077) · **Status:** in progress (the badge in `main` 3fbdd30e; the setters are DRV's) · **Blocked by:** for the second criterion only, the drivers track's `audio.wit` 1.2 (the request "The output volume and the microphone's sensitivity in `audio.wit`" in [requests-DRV.md](requests-DRV.md)) · **Main task:** — · **Constitution:** MC-3.4

## Problem

The maintainer asked for settings of the output volume and the microphone's sensitivity. The tools track (000-APP-0076, 000-APP-0077) wants them changed only by the shell:
- today `init` lends the shell a plain client of `audio_gw` in `SLOT_AUDIO`, as it lends `tts` one;
- programs get their own plain client from the loader's standard grants.

The tools track's request of 2026-10-14 is in [requests-KRN.md](requests-KRN.md).

## Plan

1. **`BADGE_AUDIO_LEVELS`** in `mind::audio`, as the clock's `BADGE_SET` (211-KRN-0051). The drivers track's `audio.wit` 1.2 serves the setters only to a client with it.
2. **`init` mints it** on the shell's client in `SLOT_AUDIO` only. `tts` and the loader's grants keep the plain client, so no program the shell starts gets it.

## Acceptance criteria

- [x] `caps` shows the shell's audio client with the badge, and the client of a program the shell starts (`say`) without it.
- [ ] With `audio.wit` 1.2, `say` cannot change the volume and the shell can (the `audio` suite, with the drivers track's check).

## Progress (2026-10-30)

Made ahead of `audio.wit` 1.2, so that the drivers track has only to check the badge:
- `mind::audio::BADGE_LEVELS` (1);
- `init` mints it on the shell's client in `SLOT_AUDIO`, beside the clock's `BADGE_SET`;
- `audio_gw` 1.1 reads no badge, so nothing changes until 1.2 serves the setters to it.

The `services` suite now checks:
- the shell's slots 2 (`rtc`) and 4 (`audio_gw`) are badged 1;
- the loader's slots 2 and 4 are plain, as are the copies programs get from them.

Left: the second criterion, with DRV's `audio.wit` 1.2.

## Landed (2026-10-30)

In round 54 (`main` 3fbdd30e). The `services` suite's badge checks passed in the gate of 485a7129 (`x86: services, storage, audio`, QEMU under TCG), which meets the first criterion. The second waits for DRV's `audio.wit` 1.2.

## Related

The tools track's 000-APP-0076 and 000-APP-0077, [requests-DRV.md](requests-DRV.md), 211-KRN-0051 (the clock's setting badge).
