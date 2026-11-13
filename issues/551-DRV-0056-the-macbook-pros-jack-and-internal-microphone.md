# 551-DRV-0056 — The MacBook Pro's codec past its speakers: the headphone jack and the internal microphone (551's step 2)

**Type:** driver · **Owner:** `DRV` · **Priority:** P2 · **Status:** in progress (2026-11-13) · **Blocked by:** — ([551-DRV-0010](551-DRV-0010-hda-controller-and-codecs.md)'s run on the MacBook Pro came on 2026-11-13) · **Main task:** [551](551-sound-on-pcs.md) · **Constitution:** MC-6.1, MC-12.1, MC-12.3

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

## Progress

**2026-11-13: the speakers heard (release 202611131315, `main` 20fba2c5).**
- **On the MacBook Pro**, the maintainer heard `beep` and `say` from the speakers, and `volume 30` (-38.5 dB) against `volume 100` (+6 dB) set through the shell. The first acceptance criterion is met there.
- **Its log:** `CIRRUS AMPLIFIERS: GPIO 0x08 OF 4 SET, READ 0x08`, `CIRRUS DIGITAL MICROPHONE ON PIN 0xE: COEFFICIENT 4 0x0007 -> 0x000F`, and `OUT PIN 0xA (SPEAKER) <- DAC 0x3; IN PIN 0xE (MICROPHONE) -> ADC 0x6; ... POLLED`. No pin table was logged, so plan step 1 had nothing to start from.

**2026-11-13: the codec's dump at start** (on `claude/551-DRV-0056-mac-jack-and-mic`).
- **`audio_gw/src/codec.rs`**, new and built on the host too: the widgets and the paths between them (moved from `hda.rs`), and their decoding as the HD Audio specification 1.0a defines it (7.3.4).
- **Logged once at start**, for every codec on the link, after the paths are set:
  - `[AUDIO] CODEC c AFG 0x1: ...`: vendor, revision, subsystem, the nodes, the group's capabilities, PCM and formats, its default amplifiers, its GPIOs (count, data, mask, direction);
  - `[AUDIO] CODEC c NID n: ...` for each widget: its type and capabilities; a pin's default configuration decoded (device, connectivity, location, connection type, colour, "no detect", association), its capabilities (presence, trigger, headphone amplifier, EAPD, VREF choices), its control, EAPD and sense; the connections and the one chosen; each amplifier's capabilities decoded and its left setting;
  - a Cirrus codec's vendor widget: `[AUDIO] CODEC c NID 0x11 COEFFICIENTS: 00=.... ...`, indices 0x00-0x1F;
  - `[AUDIO] HEADPHONE JACK: PIN n, SENSE ........: HEADPHONES IN | EMPTY`, where a headphone jack with presence detect is found beside the output.
- **Size.** A line holds at most 190 bytes and a field is never split across two; a widget takes one to three lines. QEMU's `hda-duplex` gives 6 lines; a model of the CS4206 from another Mac's published dump gives 33 (`tests/hda_codec_host.rs`), 1.6 % of logd's 2048 records. The lines go out at most 40 a second, under logd's 64 a second for one sender.
- **A late response is no answer.** Before each verb, a response left in the RIRB (one that came after its verb gave up, or an unsolicited one) is passed over, so the answers cannot shift by one (the critique's §2.3 in 551-DRV-0010).
- **Tested:** `tests/hda_codec_host.rs`; the whole `scripts/host_tests.sh`; the x86 group "services, storage, audio" on 4 CPUs (the dump's lines checked on QEMU's codec by hand).
- **Facts and where they come from:** the decoding from the HD Audio specification 1.0a; the model codec from the CS4206 dump in Launchpad question 274923 (subsystem 106B0300, not this Mac); no code is taken.

## Related

[551](551-sound-on-pcs.md), [551-DRV-0010](551-DRV-0010-hda-controller-and-codecs.md), [000-DRV-0057](000-DRV-0057-the-output-volume-and-the-microphones-gain.md).
