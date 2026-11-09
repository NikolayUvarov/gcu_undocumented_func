# 551-DRV-0056 — The MacBook Pro's codec past its speakers: the headphone jack and the internal microphone (551's step 2)

**Type:** driver · **Owner:** `DRV` · **Priority:** P2 · **Status:** open · **Blocked by:** [551-DRV-0010](551-DRV-0010-hda-controller-and-codecs.md)'s run on the MacBook Pro (the kernel session's, TRACKS 1.5) · **Main task:** [551](551-sound-on-pcs.md) · **Constitution:** MC-6.1, MC-12.1, MC-12.3

Numbered from the kernel track's request of 2026-10-09 in `requests-DRV.md` ("The MacBook Pro's speakers: more of 551's step 2 if the GPIO is not enough").

## Problem

**What exists.** 551-DRV-0010 drives Intel HD Audio and the MacBook Pro's Cirrus CS4206. The kernel session set the codec's amplifier GPIOs for Apple's machines after the maintainer's run found `beep` and `say` silent.

**What is left of 551's step 2:**
- the headphone jack, found by its presence detection, with the speaker muted while it is in;
- the internal microphone;
- any further codec setup the speakers need if the GPIOs are not enough.

## Plan

1. **Start from the Mac's own codec dump** in the boot log (551-DRV-0010 logs each pin's default configuration and connections). Take the paths from it, not from a table.
2. **The jack:**
   - the headphone pin's unsolicited response, or a poll of its presence once a second;
   - the speaker's amplifier off while it is in, and the choice logged.
3. **The internal microphone:**
   - its pin and ADC from the dump;
   - the input path's amplifier at 0 dB;
   - the bias where the pin offers it.
4. **If the speakers stay silent with the GPIOs set:** compare with Apple's published codec notes and the MIT- or BSD-licensed references, and record what is taken from where.

## Acceptance criteria

On the MacBook Pro, the maintainer's ears and the log:
- `beep` and `say` play through the speakers;
- with headphones in, they play there and not through the speakers;
- `listen` records the room through the internal microphone, at a level above silence.

## Related

[551](551-sound-on-pcs.md), [551-DRV-0010](551-DRV-0010-hda-controller-and-codecs.md), [000-DRV-0057](000-DRV-0057-the-output-volume-and-the-microphones-gain.md).
