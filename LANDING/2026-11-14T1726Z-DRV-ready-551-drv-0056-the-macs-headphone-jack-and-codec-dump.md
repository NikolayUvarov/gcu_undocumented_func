# DRV landing request: the MacBook Pro's headphone jack, and `audio_gw`'s codec dump (551-DRV-0056)

**From:** DRV · **To:** LANDING (KRN, the integrator) · **Written:** 2026-11-14 17:26 UTC · **Concerns:** 551-DRV-0056

**Ready:** `claude/551-DRV-0056-mac-jack-and-mic` 0b1181e0452c335c2c477f24591014e1073c9e6e

## What it carries

`main` 32272bba66 merged in (0b1181e0). The only conflict was `issues/README.md`, resolved keeping both sides (main's 551-DRV-0010 row, this branch's 551-DRV-0056 row). Two commits of code, both in `fast-test` already:
- **5e499100: the codec's dump at start.**
  - `audio_gw/src/codec.rs` (new, built on the host too): the widgets and paths (moved from `hda.rs`), decoded by the HD Audio specification 1.0a.
  - Once at start, for every codec: the function group, then each widget, a Cirrus vendor widget's coefficients, at most 190 bytes a line and 40 lines a second (6 lines on QEMU's `hda-duplex`).
  - A response left in the RIRB is passed over before each verb, so the answers cannot shift by one (551-DRV-0010's critique 2.3).
- **f05ca623: the headphone jack switches the speakers.**
  - `codec.rs` chooses the output, the input, the other speaker pins of the output's association and the headphone jack, from the codec's answers. Apple's amplifier GPIOs and the CS4206/4207 microphone bits are tables.
  - The jack is looked at at start and once a second. With headphones in, the speaker pins are off and the jack's pin and Apple's headphone GPIO are on; out, the other way. One line per change.
  - The jack's DAC follows the speaker's volume in dB (`levels::matching`).
  - The MacBook Pro's second speaker pin 0xB stays off. Only a build with `MIND_AUDIO_COMPANIONS` set drives it, a build switch, not an interface.
- **Tests:** `tests/hda_codec_host.rs` (new, on the Mac's own widget list and a model of the codec), `tests/audio_levels_host.rs`.
- **Text:** the issue's progress, its `issues/README.md` row.

**Issues:** advances 551-DRV-0056. Closes none: the jack and the microphone wait for the maintainer's ears on the Mac.

## What was tested, and where

LEG9, WSL2, Ubuntu 24.04, QEMU 8.2.2 (TCG), 4 CPUs, on 0b1181e0's exact tree:
- **Builds:** `./02_build.sh` (x86) and `scripts/x86_fixtures.sh`.
- **The full `scripts/host_tests.sh`** with `MIND_SSHD`: exit 0, 230 suites, 6254 passed, none failed. `hda_codec_host` and `audio_levels_host` are among them.
- **The group "x86: services, storage, audio":** red twice here, both times in the `ahci` suite, which then stops the group (below). Its other suites were then run on their own with the group's arguments: `audio`, `tts`, `listen`, `hda`, `link` and `modes` all passed. Before the `ahci` suite, `services` passed in both group runs. The `hda` suite gave `codec 1AF4:0022, line out 0X3 <- DAC 0X2, line in 0X5 -> ADC 0X4`.

**The `ahci` suite fails on this machine on `main` 32272bba66 as well,** twice in a row, the same way. It is not this branch's: the branch changes only `audio_gw` and its tests.
- **What happens:** after `kill` of `ahci` and `init`'s `ahci RESTARTED`, the suite waits for `[AHCI] PORT 0:`, then `run files &` gets `ERROR: CANNOT READ THE PROGRAM: ITS DISK DOES NOT ANSWER (UNPLUGGED?)`. `tests/qemu_smoke.py`, line 4314, then raises `TypeError: 'NoneType' object is not subscriptable`.
- **A guess** (MC-12.3, not traced): since 000-KRN-0085, `vfs_server/src/disk.rs` fails a request at once while a returned driver's medium cannot be compared yet, and tries again `RETRY_MS` (500 ms) later. The suite's one `run files &` right after `PORT 0` meets that window on this machine. KRN's 1511Z request passed the group on x32 (on `ec0264e037`), so it seems to depend on the machine's timing.

**Recorded in the issue, not run again here:** the MacBook Pro's speakers heard on release 202611131315, and the Mac's own codec dump from this branch's first commit swapped in by `replace`.

**Not tested** (MC-12.3):
- the jack's switching on the Mac: QEMU's codecs have no jack and no second speaker pin, so it ran against the model only;
- the internal microphone through `listen` on the Mac;
- a build with `MIND_AUDIO_COMPANIONS`.
