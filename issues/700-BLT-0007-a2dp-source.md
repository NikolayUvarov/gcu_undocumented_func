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
- [x] On the Ryzen: the maintainer's JBL Flip 5 played the tone ("слышны гудки") once AVRCP's absolute volume was there (diag3, 2026-11-26).
- [x] With DRV's sink (700-DRV-0110): the machine's output in the speaker. The maintainer heard `say`'s phrase («да было слышно… Фраза прозвучала»). *This was with the bench branch's `init` line for the sink client; KRN's grant is not in `main` yet.*

## Progress (2026-11-26): the Ryzen and the JBL Flip 5

**The speaker took the stream and played nothing, until it had AVRCP.**
- The stream itself was right. Once a second (diag1): 54 packets of 7 frames, all completed, none held, 0 late.
- The exact bytes for its capability `FF FF 02 28` decode with `sbcdec` to the tone (440 Hz left, 660 Hz right, −12 dBFS).
- Every AVDTP signal was accepted (diag2). The speaker's capabilities, `010007060000FFFF022804020200`, offer SCMS-T. Content protection is the source's to configure, and is not configured.
- The speaker opened AVCTP (PSM 0x0017) 3 s into the stream and registered for the playback status. The maintainer: its volume has to be given to it, and it gives its own back.

**The mend: AVRCP (avrcp.rs).**
- **Controller, category 2:** its events (`[0D]`); RegisterNotification of volume changes; SetAbsoluteVolume, 64 of 127 at first or `volume n` in the list.
- **Target answers, as far as it asks:** the playback status INTERIM and CHANGED, GetCapabilities, GetPlayStatus.
- **What it showed:** `THE SINK'S VOLUME 60 OF 127`, `THE SINK TOOK VOLUME 64 OF 127`, and its own button as `THE SINK'S VOLUME 68 OF 127 (CHANGED THERE)`, `72 …`.
- **SDP:** a controller record, checked against AVRCP 1.6.2 table 8.1 (Bluetooth SIG, read 2026-11-26), but for its profile version, kept at 0x0104: AVRCP 1.4, whose absolute volume is what the stack does. The target record of diag3 and diag4 is dropped. Table 8.2 ties category 1 to the browsing channel and all of category 1's mandatory commands, which the stack lacks.
- The bench runs had that target record. The final build, without it, is the bench's to check.

**The crackle while the mouse moved:** mended by keeping the central's role (700-BLT-0005).

**Not done.**
- The machine's volume tied to the speaker's. It would be `audio_gw`'s levels; only the start value and the speaker's own changes are logged.
- AVRCP browsing and metadata, HFP, and codecs other than SBC.
- A stream beside a 2.4 GHz station: on channel 8 the Wi-Fi lost beacons while it ran. That is 213-DRV-0052's.

## Related

[700-BLT-0005](700-BLT-0005-connections-carry-data.md), [700-BLT-0006](700-BLT-0006-classic-pairing-and-link-keys.md), [551-DRV-0010](551-DRV-0010-hda-controller-and-codecs.md) (`audio_gw`).
