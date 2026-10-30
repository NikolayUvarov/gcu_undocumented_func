# 000-APP-0114 — `hear` and `voice` wait for a whole command, and `listen` plays a quiet recording back so it is heard

**Type:** tools (`hear`, `voice`, `listen`, `mind::voice`) · **Owner:** tools track (`APP`) · **Priority:** P1 (the maintainer's report, 2026-10-30) · **Status:** in progress (done in QEMU; the MacBook Pro's run left) · **Blocked by:** — (the microphone's gain at the device: [000-APP-0077](000-APP-0077-microphone-sensitivity.md)) · **Main task:** — · **Roadmap:** track G · **Constitution:** MC-11.4, MC-12.1

## Problem

The maintainer's report (2026-10-30), from the MacBook Pro:

> `listen` plays its recording back very quietly. `hear` listens very briefly, so there is no time to say the command, and it has never recognized a command right. Perhaps the microphone's sensitivity is not set.

The causes in our code:

- **An utterance ends after 200 ms below the threshold** (`HANGOVER_FRAMES`). The pause between a command's words is often longer. At a quiet microphone a weak syllable stays below the threshold (9 dB above the noise floor) too. Either way the utterance ends after the first word, and the recognizer gets a piece of the command.
- **`hear` and `voice` take the first utterance.** A short sound before the command ends the wait: the click of the Enter key that started `hear`, or of F12 for `voice`. So does a knock.
- **`listen` plays the recording back as it was recorded.** `audio_gw` fixes the capture's gain at 0 dB. On HDA there is no boost, so the recording is quiet, and its playback too.
- **The recognizer itself does not care about the level.** It subtracts each band's mean over the utterance. What a quiet microphone costs is where the utterance is cut, and the noise relative to speech, which only the gain at the device can change (000-APP-0077, waiting for DRV's `audio.wit` 1.2).

## Plan

- **`mind::voice::Detector::with_pause(ms)`:** the quiet that ends an utterance.
  - `COMMAND_PAUSE_MS` is 600 ms for commands;
  - `Detector::new()` keeps 200 ms (`listen --vad`, the host tests);
  - the utterance's bounds are still its first and last voiced frames, so the recognizer sees what it was trained on.
- **`hear`'s library:** utterances end at the command's pause. `listen_once` takes the first utterance at least `COMMAND_MIN_MS` (300 ms) long, and hands each shorter one to its caller.
- **`hear`:**
  - it names each sound it skipped (`SKIPPED AT 0.12 S: 180 MS, LEVEL -41 DBFS, SHORTER THAN A COMMAND (300 MS)`) and waits on;
  - its first line says when an utterance ends (`… IT ENDS AFTER 600 MS OF QUIET`).
- **`voice`:** it waits for a command the same way (`[VOICE] SKIPPED …`). Asking yes or no, it takes a «да» that is shorter.
- **`listen`:**
  - it plays the recording back raised to a peak near full scale (about -1 dBFS), at most 32 times (+30 dB), never lowered (`mind::voice::playback_gain`);
  - it says so (`[LISTEN] PLAYBACK RAISED 9.6 TIMES: THE PEAK FROM -20 DBFS TO -1 DBFS`);
  - the recorded level in `[LISTEN] RECORDED …` stays the microphone's own.
- **Docs:** `docs/voice/README.md` (EN, RU), `hear`'s and `listen`'s help.

## Acceptance criteria

- **Host tests** (`tests/voice_host.rs`):
  - two words about 400 ms apart are one utterance at the command's pause and two at 200 ms; a second apart, two;
  - the playback gain: as recorded at and above -1 dBFS, raised up to 32 times, never lowered;
  - the existing detection tests pass unchanged.
- **QEMU `listen` suite:** `hear --wav` and `voice on --wav` on the host-made WAVs, phrases a second apart, recognize as before.
- **QEMU `audio` suite:** `listen`'s recording and playback as before; the input there is silence, so nothing is raised.
- **On the MacBook Pro** (the maintainer's run, or LNK's through `effector485`):
  - `hear` takes a whole command said at arm's length;
  - `listen`'s playback is heard;
  - `hear --save data/media/heard.wav` keeps what it heard, for APP to study through Effector's `collect` (the owner's policy collects `data/media/`).

## Progress (2026-10-30)

- **Done** as planned, in b845f70b.
- **Host tests** (`tests/voice_host.rs`, 22 tests) passed, with `a_commands_pause_keeps_its_words_together`:
  - two words 300–500 ms apart are one utterance at 600 ms and two at 200 ms, and a second apart they are two;
  - `playback_gain` was checked at 0, at full scale and at its limits.
- **The QEMU `listen` suite** passed on x86 with 4 CPUs (TCG, this session's container): `hear --wav` and `voice on --wav` recognize what they did before, phrases a second apart.
- **The QEMU `audio` suite** passed on its second run. The first failed on `beep`'s notes, 0.676 s against 0.55–0.65 s in the recorded sound, at a load average of 4.4 in the container. This change does not touch `beep` or `audio_gw`.
- **Left:** the MacBook Pro.
  - The maintainer's run: `hear`, then a command at arm's length; and `listen 3`, whose playback should now be heard.
  - With `hear --save data/media/heard.wav`, APP can collect the recording through Effector and see what the microphone gives.

**Progress (2026-10-30): the `listen` suite.** Its 1 s recording overflowed `audio_gw`'s capture ring in 7 runs of 13 on x86 TCG with 4 CPUs, with `main`'s own `listen` as with this one, and with the check typing nothing as with its polling. The playback this task changed comes after the recording. The overflow is recorded for DRV in [requests-DRV.md](requests-DRV.md#audio_gw-the-ac97-capture-ring-overflows-in-the-listen-suite-on-x86-tcg-for-000-app-0114), and the check's requirement stays.

## Related

[000-APP-0077](000-APP-0077-microphone-sensitivity.md) and [000-APP-0076](000-APP-0076-output-volume.md) (the levels at the device), [000-APP-0053](000-APP-0053-hear-shows-what-it-heard.md) (`hear`'s levels), [000-APP-0052](000-APP-0052-voice-control-as-a-service.md), [079](../issues-done/079-voice-control-in-the-shell.done).
