# 700-BLT-0007 — Audio out: an A2DP source with our own SBC encoder, a tone first, the audio gateway next

**Type:** library (`bt_host`) · **Owner:** `BLT` · **Priority:** P1 (headphones are the first device the maintainer named) · **Status:** in progress (2026-11-26) · **Blocked by:** `DRV`: a sink in `audio_gw` that hands its output to `bt_hci` (for `say` through headphones; the tone needs nothing) · **Main task:** [700](700-bluetooth.md) · **Constitution:** MC-12.1, MC-12.2, MC-12.3

## Problem

Headphones need the A2DP profile, its codec SBC (mandatory for every sink), AVDTP's signalling and a media channel. `bt_host` had none of these.

## Plan

1. **SBC encoder (`sbc.rs`).** Written from the A2DP 1.3.2 specification's Appendix B, not from libsbc or BlueZ.
   - The analysis filter of figure 12.5 in fixed point. Its prototypes are tables 12.23 and 12.24, scaled by 2^30, taken from the specification's text.
   - Scale factors, joint stereo by the simple criterion of 12.7.3, and the bit allocation of 12.6.3 exactly.
   - Quantization, the frame syntax and CRC-8.
   - Every mode, block length, subband count and allocation.
2. **SDP server (`sdp.rs`).** It answers a sink's searches with our A2DP source record: L2CAP PSM 0x0019, AVDTP 1.3, A2DP 1.3. Long answers are continued.
3. **AVDTP (`avdtp.rs`), one audio source endpoint (SEID 1).**
   - **As initiator:** discover, get capabilities, set configuration, open the media channel, start. The configuration prefers 48 kHz, joint stereo, 16 blocks, 8 subbands, loudness and the highest bitpool up to 53 within the sink's range.
   - **As acceptor:** we answer discover, get (all) capabilities, set and get configuration, open, start, suspend, close, abort and delay report. Reconfigure and security control are refused.
4. **Media.** RTP headers (payload type 96, the timestamp in samples) and SBC frames, as many as the media channel's MTU takes.
   - Paced by the clock, 40 ms ahead, and never more than 6 packets queued for the controller.
   - A stall of more than a second is skipped, not sent late.
5. **The tone.** 440 Hz left and 660 Hz right, 0.4 s of every second. It is the first source, so the path can be heard before any program feeds it.
6. **`audio_gw`.** A sink in `audio_gw` (DRV's) hands its 48 kHz PCM to `bt_hci`, so that `say` plays in the headphones. Its shape is DRV's to settle; until then the tone.
   - `bt_host`'s side is ready: `Stack::audio(samples)` takes 16-bit stereo 48 kHz PCM into a quarter second's ring.
   - `Stack::audio_wanted()` says how many frames would keep a tenth of a second waiting while a stream runs.
   - While PCM came in the last second, it replaces the tone. A gap plays as silence. At 44.1 kHz it is read by linear interpolation (the testing stage's resampler).
   - The sink is 700-DRV-0110 (`audio.wit` 1.3). `bt_hci` takes it while its stream to headphones listed as `audio` runs, reads the output a tenth of a second ahead, and gives it back when the stream stops. While it is held, a gap is silence, never the tone. `audio tone` keeps the tone.

**What is not done.**
- AVRCP (the headphones' buttons).
- HFP (a headset's microphone, a sensor: MC-11.4).
- Codecs other than SBC.
- Content protection.
- A2DP sink.

## Acceptance criteria

- [x] SBC bit-exact against vectors checked by an independent decoder (`bt_host/tests/a2dp.rs`, `tests/data/sbc_*.hex`).
  - Each second of four configurations was decoded with `sbcdec` (libsbc 2.0, Ubuntu's sbc-tools, used as a tool only).
  - Against the input, the output lags by the filter bank's 73 or 37 samples.
  - SNR: 52.6, 48.3, 52.5 and 54.1 dB.
  - A sweep of 13 configurations through the same decoder all decoded, with the lag expected (2026-11-26, local).
- [x] Host tests:
  - the CRC and the frame length;
  - the bit allocation;
  - the configuration chosen and accepted;
  - AVDTP as initiator, down to a second of paced media packets;
  - AVDTP as acceptor, with a refusal and general reject;
  - SDP searches, attributes, continuation and errors (`bt_host/tests/sdp_hid.rs`).
- [ ] On the Ryzen: the maintainer's headphones play the tone (`A2DP: STREAMING`, `A2DP: SBC … BITPOOL …`, heard in both ears).
- [ ] With DRV's sink: `say` in the headphones.

## Related

[700-BLT-0005](700-BLT-0005-connections-carry-data.md), [700-BLT-0006](700-BLT-0006-classic-pairing-and-link-keys.md), [551-DRV-0010](551-DRV-0010-hda-controller-and-codecs.md) (`audio_gw`).
