# APP ready: the levels in the shell and `wm`'s Sound page, `tts`'s neural voice, `ip`'s OFF, and the listen suite's waits

**Ready:** `claude/wizardly-franklin-kec1a9` 400514c42374d0ebe2628aa30aa580f595a3fe69

**From:** APP · It contains `main` 7c496688 (round 110, 252-KRN-0230 included).

**What it lands:**
- **000-APP-0076 and 000-APP-0077** (stay open for the MacBook Pro's checks):
  - **The shell:** `volume` and `mic`, through its `BADGE_LEVELS` client (000-KRN-0118, 000-DRV-0057). They are kept in `data/sound.conf` and set again at the shell's start.
  - **`wm`'s Sound page:**
    - sliders and the mute and boost boxes, set through the shell's commands;
    - a test tone after a change of the volume, through `wm`'s own client, which only plays;
    - a live meter of the microphone, recorded only while the page shows (`wm/src/meter.rs`).
  - **`hear`** names the gain it hears at.
  - **Docs:** `docs/tools` and `docs/voice` (EN, RU).
- **000-APP-0048 done:** every Settings page, the Sound page last.
- **252-APP-0128** (stays open):
  - `tts` speaks Russian with Vosk TTS 0.7 from `models:` where `init` lends it, and the formant voice otherwise;
  - it is built for the float target, and `02_build.sh` stages the newer of a crate's two builds;
  - the named `ttsneural` suite;
  - the `tts` suite takes either reason for the formant voice, so it passes with KRN's empty model disk.
- **550-APP-0129 done:** `ip` names a card out of use (`OFF`, `socket.wit` 2.11).
- **000-APP-0131** (stays open for its 20 runs beside a load):
  - the listen suite waits up to 60 s for an answer whose text it checks;
  - `hear` asks for the microphone only once `listen --vad 60` holds it.

**Tested** in this track's container, QEMU, 4 CPUs, TCG:
- **x86:**
  - the audio, wm, shell and net suites on 979ddeba (72 checks);
  - the audio, wm, listen and tts suites on 30590f10 (22 checks);
  - after `main` 7c496688 was merged in (6894fa27): the tts suite with KRN's holdings check, and `ttsneural` on `main`'s `init` with no local edit (the voice heard as `говорит разум корабля`).
- **aarch64:** the build with `--fixtures`, and the normal and shell suites, on 632573e7.
- **Host:** `scripts/host_tests.sh`: every test passed but one:
  - `tests/effector_server_test.py`'s `test_contract_tls_pin_auth_and_prefix`, which fails here on `main` too (Python 3.13.18);
  - EFF is told (`EFF/…-APP-effector-server-test-fails-on-python-3-13-here-on-main-too.md`).
- **Not run:** the other groups.
