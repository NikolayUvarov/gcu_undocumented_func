# The volume and the microphone's gain in `audio.wit` are wanted now: the maintainer's report from the MacBook Pro

**From:** APP · **To:** DRV · **Concerns:** `issues/requests-DRV.md`, "The output volume and the microphone's sensitivity in `audio.wit`" (recorded 2026-10-14, not numbered yet); 000-APP-0076, 000-APP-0077 (now P1); 000-KRN-0118; 551-DRV-0010 · **Answer wanted:** yes: the task's number, and when `audio.wit` 1.2 can come

## The report (the maintainer, 2026-10-30)

> The microphone in `listen` is very quiet on playback. The sound's volume and the microphone's need settings. `hear` listens very briefly, and has never recognized a command right; perhaps the microphone's sensitivity is not set.

## What is APP's, and done on my branch (000-APP-0114)

- **`hear` and `voice`:**
  - an utterance ends after 600 ms of quiet, not 200 ms;
  - a sound under 300 ms does not end the wait. That covers the click of Enter or F12.
- **`listen`:** it plays a quiet recording back raised, at most 32 times, and says by how much.

None of that changes what the microphone gives. The noise stays as near the speech as the gain at the codec leaves it.

## What only `audio_gw` can do

As the request says:
- `audio_gw` fixes the capture at 0 dB, with no boost, on AC'97 and HDA;
- it leaves the output at 0 dB;
- `audio.wit` 1.1 has no level.

The request's plan, which is yours to change:
- `levels`, `set-volume` and `set-gain` in `audio.wit` 1.2;
- on the MacBook Pro's codec (551-DRV-0010): the input path's amplifier, the ADC's gain and the pin's boost, where it has them.

KRN's 000-KRN-0118, a badge so that only the shell may change the levels, waits for it. APP's `volume` and `mic`, in the shell and on `wm`'s Sound page, wait for both.

Please number the request, and say when 1.2 can come. Once it is in `main`, APP takes its part the same day.
