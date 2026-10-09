# 551-DRV-0010 — Intel HD Audio in `audio_gw`: controller, codecs, playback and capture

**Type:** driver · **Owner:** `DRV` (open; made by the kernel session for 551) · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Main task:** [551](551-sound-on-pcs.md) · **Constitution:** MC-1.5, MC-6.3, MC-11.4

## Problem

`audio_gw` drives only AC97. Real PCs and the MacBook Pro have Intel High Definition Audio (PCI class 04:03), so the voice programs have no microphone and no output there (551).

## Plan

- **init** grants `audio_gw` the first HDA controller's BAR0, its interrupt (MSI-X or MSI where the kernel offers it, else the line) and a DMA region, as it grants AC97's today. With both an HDA and an AC97 card present, HDA wins.
- **The controller:**
  - reset;
  - CORB and RIRB in the DMA region, for verbs and their responses;
  - the codecs found from STATESTS;
  - one output and one input stream descriptor, each with its buffer descriptor list;
  - stream reset, 48 kHz 16-bit stereo format, the run bit;
  - position from the stream's LPIB;
  - the interrupt on each buffer completion.
- **The codecs:**
  - walk the audio function group's widgets (audio output and input converters, pins, mixers and selectors);
  - read each pin's default configuration (device: speaker, headphone, line out, microphone; connectivity: jack, internal, none) and its connection lists;
  - choose an output path (internal speaker, else headphone, else line out) and an input path (internal microphone, else microphone jack);
  - set the converter's stream and format, unmute the amplifiers along the paths and enable the pins (EAPD where a pin has it).
- **Interface:** `idl/audio.wit` unchanged.
  - `play` and `wait` feed the output stream from the lent buffer.
  - `record-start`, `record-read` and `record-stop` read the input stream; one owner as now.
  - `device` says HDA.
- **Restart (MC-6.3):** the stream run bits clear and the controller is reset before a new instance starts; the DMA region is cleared, as for the other drivers.
- **Log:** the controller, the codecs (vendor and device IDs, revision) and the chosen pins with their configurations, for the hardware report.

## Acceptance criteria

- **QEMU x86:** `-device intel-hda -device hda-duplex` and `-device ich9-intel-hda -device hda-micro`. The `audio` suite's tones arrive in QEMU's wav backend, and `listen` records the input QEMU feeds. AC97 still passes.
- **The MacBook Pro:** the controller and the CS4206 are found and logged. Speaker output and microphone input are 551's next step.

## Related

[551](551-sound-on-pcs.md), the AC97 path in `audio_gw/src/main.rs`.
