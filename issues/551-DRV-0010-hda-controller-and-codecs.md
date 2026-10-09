# 551-DRV-0010 — Intel HD Audio in `audio_gw`: controller, codecs, playback and capture

**Type:** driver · **Owner:** `DRV` (made by the kernel session for 551; it stays with that session until done, TRACKS 1.5) · **Priority:** P1 · **Status:** in progress (done in QEMU; the MacBook Pro's run left) · **Blocked by:** — · **Main task:** [551](551-sound-on-pcs.md) · **Constitution:** MC-1.5, MC-6.3, MC-11.4

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

## Progress

**2026-10-09: HDA plays and records in QEMU.**

- **init** grants the first HDA controller's BAR0, its line where it has one below 16, and a 204 KiB DMA region; HDA wins over AC97.
- **`audio_gw/src/hda.rs`:**
  - **Controller:** reset; CORB and RIRB in the DMA region. RIRBCTL's response flag is on (CIE off), because a controller stops taking verbs after RINTCNT responses until that flag is cleared; QEMU's does, and the first boot hung on it.
  - **Codec walk:** the function group and its widgets, connection lists (short and long form, ranges), pin capabilities and default configurations.
  - **Output path:** an internal speaker, else headphones, else line out, to a DAC; amplifiers unmuted at 0 dB, selectors set, the pin enabled, EAPD where the pin has it.
  - **Input path:** an internal microphone, else a microphone jack, else line in, from an ADC; the microphone pin's bias at 80 % where it offers it.
  - **Streams:** one output and one input, each cyclic over its buffer list. Played buffers are cleared, so a ring the writer left plays silence. The position comes from LPIB, read on the interrupt or by the gateway's 20 ms polling while a stream runs.
- **`audio_gw/src/main.rs`:** a `Device` of HDA or AC97 behind the same `idl/audio.wit`. It logs `[AUDIO] HDA READY: CODEC c VVVV:DDDD REVISION …; OUT PIN … <- DAC …; IN PIN … -> ADC …`.
- **Tests:** the `hda` suite (x86, CI group "services, storage, audio") runs QEMU's `intel-hda` with `hda-duplex`. beep's demo tones come out in the wav backend, and `listen` records 48000 frames. The `audio` and `listen` suites still pass on AC97.

Left: the MacBook Pro (its CS4206 found and logged, the speaker amplifier and the internal microphone are 551's step 2).

**2026-10-09: silent on the MacBook Pro (fast-test 57242b9c78ff).**
- **What ran.** The controller and the codec were found: `CODEC 0 1013:4206`, speaker pin 0xA fed by DAC 0x3. `say` ran its stream to the end (`[TTS] SPOKE 5 BYTES, 705 MS`), yet neither `beep` nor `say` was heard.
- **The cause, by Linux's facts.** Apple's Cirrus codecs switch the speaker amplifier on through the function group's GPIO3, and the headphone amplifier through GPIO1 on the MacBook Pro 10,1 (subsystem 106B:2800) or GPIO2 on the others. Nothing set them.
- **The change**, made here ahead of step 2 because the maintainer's run needs it: for a Cirrus codec with Apple's subsystem ID, `audio_gw` sets those GPIOs (mask, direction, data: speakers, or headphones when the output pin is a headphone jack). It logs `HDA: APPLE <SSID>, CIRRUS AMPLIFIERS: GPIO … OF <count> SET, READ …`.
- **Left.** The MacBook Pro's run. If it is still silent, the next thing to try is Cirrus's errata coefficients for the vendor widget 0x11. Jack detection and the internal microphone stay with step 2, which `DRV`'s owner numbers.


**2026-10-09, later: sound on the MacBook Pro, the microphone silent, clicks** (fast-test fe6e7e250512).
- **Sound.** The amplifier GPIO took (`GPIO 0x08 OF 4 SET, READ 0x08`), and speech is heard. But Russian speech is barely intelligible and clicks. The clicks are this task's to find, below; the synthesizer's intelligibility is `APP`'s ([requests-APP.md](requests-APP.md)).
- **The microphone.** No program that listens worked.
  - By Linux's facts, the MacBook Pro 10,1's internal microphone is a digital one on the CS4206's pin 0xE (DMIC1). It is switched on by bit 3 of coefficient 4 of the vendor widget 0x11, and nothing set it.
  - `audio_gw` now sets it, and logs the coefficient before and after.
- **Capture in the log.** Each capture logs:
  - its start, with the client's PID, or the refusal;
  - the first second's peak (`SILENT: THE MICROPHONE GIVES NOTHING` under 64);
  - `NO DATA` when the input stream gives nothing for 2 s;
  - at its stop, how much sound it gave and how many overflows.
- **The clicks, to find out.**
  - The Mac's controller runs `POLLED`, without an interrupt line, at a 20 ms poll against 21 ms buffers. A late poll lets the ring run dry, and the controller plays stale buffers.
  - Next: count the underruns in the log (debug mode), take the interrupt (MSI) where the line is missing, or poll at 5 ms while a stream plays, and listen again on the Mac.
  - **Measured in QEMU (2026-10-09).** The `tts` suite's recording has no silence of a buffer's length inside the speech, so the driver's ring did not run dry there. What it does have is the synthesizer's: onsets rising to 3000–5000 within two or three samples after its pauses, and the linear upsampler's images at 8–12 kHz, only 8 dB below the sibilants. Both are recorded for the tools track (requests-APP.md, "Russian speech is barely intelligible"). The maintainer also hears a periodic hiss in the sounds, which fits those images. The Mac's polled ring is still to be counted.


## Related

[551](551-sound-on-pcs.md), the AC97 path in `audio_gw/src/main.rs`.
