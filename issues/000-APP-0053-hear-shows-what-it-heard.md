# 000-APP-0053 — `hear` shows what it heard, for the operator

**Type:** tools · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** in progress (done in QEMU; the MacBook Pro's microphone left) · **Blocked by:** — · **Main task:** — · **Roadmap:** track G · **Constitution:** MC-10.2

Numbered on 2026-10-10 from `requests-APP.md`, where the kernel track recorded it.

**Recorded by:** the kernel track (KRN), 2026-10-10, at the maintainer's request.

## Problem

The maintainer: "`hear` prints only errors now; it should print what it heard, so the operator can check it."

What it prints today:

- `HEARD "<phrase>" …` for an accepted command;
- `NOT UNDERSTOOD (CLOSEST "…", CONFIDENCE=…)` for a refused one;
- `NOT UNDERSTOOD (TOO SHORT)`, `NOTHING HEARD`;
- `HEAR: NO MICROPHONE …`, `HEAR: NO INPUT FROM THE MICROPHONE`, or `HEAR: THE MICROPHONE IS BUSY …`.

When the microphone gives silence or noise, every one of these reads as an error. Nothing tells the operator whether sound came in at all, how loud it was, or what the recognizer made of it.

## Plan

- **While it listens:** a level meter on one line (the peak and RMS in dBFS, refreshed several times a second), and the threshold that starts an utterance.
- **For each utterance:** its length, its level, and the best three phrases with their confidence, then the verdict: accepted, refused below the threshold, or too short.
- **When nothing was heard:** the noise floor's level over the wait, so that "the microphone gives nothing" and "it was too quiet" are told apart.
- **`--save FILE`** keeps the utterance (or the whole wait) as a WAV file on `ram:` or `log:`, for the operator to send for analysis or to feed back with `hear --wav`. (`listen` already shows a level and plays a recording back, but separately from recognition.)
- **`voice`** prints the same per-utterance lines on its console.

## Acceptance criteria

- In QEMU, `hear --wav` of the test recordings prints the level, the length and three candidates for each utterance. With no input it prints the noise floor.
- On the MacBook Pro, the operator can see whether the microphone gives sound (551-DRV-0010 logs the capture's peak on the driver's side).

## Progress

- **`mind::voice`:**
  - `meter` gives a piece's peak and RMS in dBFS;
  - `Detector::threshold_dbfs` gives the level that starts speech;
  - `Grammar::rank_where` and `best` rank the phrases;
  - `Recognizer::recognize_ranked` gives the decoding and the best phrases with their confidence;
  - `Recognizer::refusal` says why a decoding is refused.
- **`hear`:**
  - a meter line every 250 ms while it listens;
  - `UTTERANCE AT … S: … MS, LEVEL … DBFS` and `CANDIDATES:` with three phrases for each utterance;
  - the refusal's reason after `NOT UNDERSTOOD (…)`;
  - `NOTHING HEARD:` with the noise floor and the peak, or digital silence;
  - `--save FILE`.
- **Authority.** `hear` asks for the user's files for `--save`. It drops them at start without it, and with it once the file is written, before recognizing.
- **`voice`** logs `[VOICE] <ms> MS <level> DBFS: "<phrase>" <confidence>, …` for each utterance.
- **Host tests** (`tests/voice_host.rs`) check the meter, the threshold and the ranked candidates.
- **The `listen` suite** checks the lines, `--save` and the noise floor. Each utterance of the test recording has its time, length, level and three candidates; a refusal names its reason. The saved first utterance is heard again. Silence gives the meter line and the noise floor, and `voice` logs its candidates. It passed in QEMU and in the local gate (2026-10-10).
- **Left:** on the MacBook Pro, the operator sees whether the microphone gives sound, once capture works there (551-DRV-0010).

## Related

[requests-APP.md](requests-APP.md) (where it was recorded).
