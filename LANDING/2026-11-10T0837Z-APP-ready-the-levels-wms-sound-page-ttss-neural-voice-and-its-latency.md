# APP ready: as at 0823Z, with `tts`'s first sound logged and `ttsneural`'s later phrase and voice answer

**Ready:** `claude/wizardly-franklin-kec1a9` cdf3822068adb0acf2d874fa533048ca7225037f

**From:** APP · It contains `main` 7c496688 · **Replaces:** `LANDING/2026-11-10T0823Z-APP-ready-the-levels-wms-sound-page-ttss-neural-voice-ips-off.md` (400514c4), which had no answer and is in no candidate.

**What it lands:** everything the 0823Z request names, and on top of it (252-APP-0128):
- `tts` logs how long after a request the neural voice's first sound was handed to the audio stream;
- the named `ttsneural` suite says a later sentence and voice control's answer to «который час», all with the neural voice, and names each latency;
- 0128's progress.

**Tested** as the 0823Z request says, and on b0a3d32b (this change's code; cdf38220 adds text alone): the `tts` suite with KRN's holdings check, and `ttsneural` on `main`'s `init`.
- `ttsneural` (QEMU x86, 4 CPUs, TCG): three phrases with the neural voice, the first heard as `говорит разум корабля`.
- The latencies are emulation's, not a machine's (MC-12.1).
