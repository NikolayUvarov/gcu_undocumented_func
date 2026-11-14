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
- **Fast-test:** merged as 189896f2.

**2026-11-13: the Mac's own dump, and its speakers' channels.**
- **The dump.** The kernel session ran this branch's `audio_gw` on the MacBook Pro (built on the Mac's release tree, ABI 32, and swapped in over the link with `replace unit=audio_gw`, until its next boot). 33 lines of the dump, the facts:
  - **the headphone jack** is pin 0x9: configuration 002B4020 (headphone, jack, external, green, association 2.0), capabilities 1C (presence, headphone amplifier, out), fed by DAC 0x2, which the firmware left muted (0xF3); sense 00000000 with nothing plugged in;
  - **two speaker pins of association 1:** 0xA (90100112, sequence 2) from DAC 0x3, the one driven; and 0xB (90100110, sequence 0) from DAC 0x4, left off by the firmware (control 0x00, its DAC muted);
  - **the microphone** is pin 0xE (90A60100: internal, digital), capabilities 20 (in, no bias to offer), to ADC 0x6 through its second connection; the ADC's amplifier at 0 dB (0x33), the pin's boost at 0 dB;
  - **the GPIOs:** 4, data 0x08, mask 0x0A, direction 0x0A; the vendor widget's coefficients 00=001E, 01=0008, 02=000A, 03=106A, 04=000F, the rest 0.
- **Heard by the maintainer** (2026-11-13, 14:23-14:25 UTC): with pin 0xA alone driven, a beep that zeroes one channel (left 440 Hz, right 660 Hz) gives left and right clearly apart ("левый правый четко различимы"). So 0xA alone is the stereo pair. What 0xB adds (a woofer, a second pair) is not known yet.

**2026-11-13: the jack, the speakers, the microphone** (on `claude/551-DRV-0056-mac-jack-and-mic`).
- **`codec.rs` now chooses, from the codec's own answers** (host-tested on the Mac's widget list above):
  - the output and the input (`output`, `input`, as `hda.rs` chose them before);
  - the other speaker pins of the output's association (`companions`, associations 1 to 14);
  - the headphone jack beside a speaker output (`jack_path`): a headphone pin, a jack that senses presence (no "no detect"), that a DAC reaches. A headphone, line-out or line-in output gets none.
- **The jack** (`Jack::watch`, `route`):
  - **set up:** its path unmuted at 0 dB, its DAC on the output stream beside the speaker's;
  - **looked at:** at start, then once a second from the gateway's loop (pin sense F09, triggered first where the pin needs it);
  - **headphones in:** the speaker pins' control 0 (EAPD off where they have it), the jack's pin 0xC0 (0x40 without a headphone amplifier), Apple's GPIO the headphones' bit alone (0x02 on this Mac);
  - **headphones out:** the other way round (speaker pins 0x40, GPIO 0x08);
  - **logged** once at start and once per change: `[AUDIO] HEADPHONES IN (PIN 0x9, SENSE ...): THEY PLAY FROM DAC 0x2 (CONTROL 0xC0); THE SPEAKER PINS 0xA OFF, GPIO 0x02`, and `HEADPHONES OUT ...: THE SPEAKER PINS 0xA PLAY (CONTROL 0x40); THE JACK'S PIN OFF, GPIO 0x08`. Phase 1's `HEADPHONE JACK:` line is gone.
- **The volume:** the amplifier nearest the jack's DAC follows the speaker's setting in dB (`levels::matching`; the same setting on the Mac, whose DACs are alike) and is muted with it.
- **The second speaker pin (0xB) stays off by default,** since 0xA alone gives the stereo pair. A build with `MIND_AUDIO_COMPANIONS` set (any value) drives the other speaker pins of the association, each from its own DAC on the same stream, their volume following. Without it the gateway logs `[AUDIO] HDA: SPEAKER PIN 0xB OF PIN 0xA'S ASSOCIATION 1 LEFT OFF (A BUILD WITH MIND_AUDIO_COMPANIONS DRIVES IT)`. It is a build switch: `audio_gw` holds no VFS client to read a setting with (`docs/profile/bootstrap.md`), and a call to set it would be a new `audio.wit` version. It exists to find out by ear what 0xB is.
- **Apple's amplifiers in a table** (551-DRV-0010's critique): `codec::apple_amplifiers`, by subsystem. By Linux 6.1's `patch_cirrus`, the headphones are GPIO2 only on the iMacs 12,1, 12,2 and 14,1 (106B0900, 106B2000, 106B0600) and GPIO1 on every other Apple machine. The old code gave GPIO2 to every machine but the MacBook Pro 10,1; this Mac's GPIO1 is unchanged. The digital microphones' coefficient bits are in a table too (`cirrus_digital_microphone`).
- **The microphone:** nothing changed on the Mac's path. The dump shows it at 0 dB with DMIC1 on, and pin 0xE offers no bias.
- **Tested:**
  - `tests/hda_codec_host.rs` (10 tests): on the Mac's own widget list, the paths (0xA from 0x3, 0xB from 0x4, the jack 0x9 from 0x2, 0xE to ADC 0x6), and the verbs the jack's changes send to a model of the codec (in, out, no change; EAPD and a triggered sense on another layout; no GPIO off Apple's machines);
  - `tests/audio_levels_host.rs` (`matching`);
  - the whole `scripts/host_tests.sh`; the x86 group "services, storage, audio" on 4 CPUs.
  - QEMU's codecs have no headphone jack and no second speaker pin. So the switching has run only against the model, and in QEMU only its absence (the same lines as before) was checked.
- **Left for the Mac** (MC-12.3, nothing above is claimed there yet). Two builds wait with the kernel session for the Mac: the plain one, and one with `MIND_AUDIO_COMPANIONS`, both on the ABI 32 tree. They wait for the link's cable to be back on the Mac (it went to the Ryzen) and for headphones at hand. Then the maintainer's ears are to say:
  - with headphones in, the sound is in them and not in the speakers, and `volume` changes them;
  - pulled out, the speakers play again;
  - `listen` records the room above silence;
  - what 0xB adds to the left/right beep.
- **Facts and where they come from:**
  - the HD Audio specification 1.0a;
  - Linux 6.1's `patch_cirrus` (sources.debian.org): the Apple quirk table and its GPIO fixups, the GPIO data with headphones in or out, coefficient 4's DMIC bits, the coefficients' names. Facts only; no code is taken;
  - the Mac's dump above, and Launchpad question 274923's dump of another Mac's CS4206.

## Related

[551](551-sound-on-pcs.md), [551-DRV-0010](551-DRV-0010-hda-controller-and-codecs.md), [000-DRV-0057](000-DRV-0057-the-output-volume-and-the-microphones-gain.md).
