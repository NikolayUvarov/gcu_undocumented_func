# 551 — Sound on real PCs and the MacBook Pro: Intel HD Audio in the audio gateway

**Type:** drivers (main task) · **Owner:** `DRV` (open; worked by the kernel session at the maintainer's request) · **Priority:** P1 · **Status:** open (plan) · **Blocked by:** — · **Roadmap:** tracks A and G · **Constitution:** MC-1.5, MC-10.2, MC-11.4 (a microphone is a sensor of the user's surroundings)

## Problem

The maintainer asked on 2026-10-09 that the mind be able to hear and speak on the test MacBook Pro, which has speakers, a microphone and a camera.

- **Who uses sound.** The voice programs `listen`, `hear` and `voice` use the microphone through `audio_gw`'s `record-*` (`idl/audio.wit` 1.1), and `tts` and `beep` play through it.
- **What `audio_gw` drives.** Only AC97, QEMU's legacy card. On the MacBook Pro it logs `[AUDIO] NO AC97 DEVICE; GATEWAY ANSWERS WITHOUT OUTPUT` (LOG:boot0001.log). Every PC since about 2005, the MacBook Pro included, has Intel High Definition Audio (PCI class 04:03) instead: an HDA controller with one or more codecs (the MacBook Pro: Cirrus Logic CS4206, by Linux reports).
- **So** the mind can neither hear nor speak on real hardware.

## Plan

The gateway keeps its interface: programs see a microphone and an output whatever drives them.

| Step | Task | What |
|---|---|---|
| 1 | [551-DRV-0010](551-DRV-0010-hda-controller-and-codecs.md) | **HDA in `audio_gw`**, tested in QEMU (`intel-hda` and `ich9-intel-hda` with `hda-duplex` or `hda-micro`). It covers the controller (CORB/RIRB, stream descriptors and buffer descriptor lists, MSI or the line) and the codec graph (function groups, widgets, pin defaults, connections, amplifiers). It picks the paths from the output pins to speakers or headphones, and from the microphone pins to a converter. Playback and capture serve `idl/audio.wit` unchanged; AC97 stays for QEMU's legacy card. The codec's names and widgets go to the hardware report (174-KRN-0038). |
| 2 | (numbered when started) | **The MacBook Pro's codec.** Speakers (the amplifier behind a GPIO), headphones with jack detection, and the internal microphone, from the codec's pin configuration as the firmware left it. Linux's fixups are GPL, so only the facts are taken: pins, GPIOs, verbs. |
| 3 | (numbered when started) | **Common PCs:** Realtek ALC codecs and AMD's HDA controllers, each machine recorded in the profile. |

**Activation**, as the maintainer set it:
- **For now** the mind hears and sees when a program is started explicitly: `listen` / `hear` / `voice` for hearing and speech, `camera` for sight. Each needs the user's consent where the shell asks for it (the microphone, the camera, 158).
- **Later** the same programs run as services that the user starts by hand, through the boot service configuration (173: a service listed but not started at boot).

## Acceptance criteria

- **QEMU:** with `intel-hda` and a duplex codec:
  - `beep` and `tts` play through the HDA path;
  - `listen` records what the host feeds the codec's input;
  - the `audio`, `tts` and `listen` suites pass on HDA as on AC97.
- **The MacBook Pro:**
  - the speakers and the headphone jack play `say` and `tts`;
  - `listen` hears the internal microphone;
  - the hardware report names the codec.

## Related

[158](158-video-capture.md) (the camera), [174](174-full-use-of-pc-hardware.md), [211](211-intel-pc-from-a-sata-ssd.md), the voice plan [docs/voice](../docs/voice/README.md).
