# DRV → LANDING: ready, the output volume and the microphone's gain in `audio.wit` 1.2 (000-DRV-0057)

**Ready:** `claude/ASR-DRV` 6f3cae36

**On `main`** b1e085a2 (a fast-forward from it).

## What it changes

- **`idl/audio.wit` 1.2 (minor):**
  - `levels`: the volume, mute, gain and boost in per cent, the boost in quarter-dB, and a `Range` for each side (steps, step, zero, scaled);
  - `set-volume(percent, muted)` and `set-gain(percent, boost)`.

  The bindings are regenerated, and `Range` and `Levels` are in `idl_fuzz_host`.
- **`audio_gw`:**
  - **AC'97:** the volume on master 0x02, its width found at start; the gain on record gain 0x1C; the boost on the microphone's +20 dB, 0x0E bit 6.
  - **HDA:** the amplifier nearest the DAC and the ADC on each path, and the input pin's for the boost.
  - **Without an amplifier,** it scales the samples, and `levels` says `scaled`.
  - **Amplifier capabilities** are read from the widget or the function group, by the override bit. This touches the existing 0 dB unmute too.
- **Who may set:** only a client badged `BADGE_LEVELS`, the shell's (000-KRN-0118). Others get `rights`, which is logged; `levels` answers every client.
- **`libmind/src/audio.rs`:** the three calls.
- **Tests:** `tests/audio_levels_host.rs`, and `tests/levels_stub` in the audio suite. Docs: `docs/idl`, `docs/voice` in English and Russian.

## What was tested, where

On this machine:
- `./02_build.sh`;
- the whole `scripts/host_tests.sh` with `MIND_SSHD`: 197 suites, none failed (`audio_levels` 5);
- `scripts/ci_local.sh --only x86`, both PASS:
  - `build (x86 test programs)`;
  - `x86: services, storage, audio`:
    - AC'97's levels at start and the master's 64 settings;
    - HDA (QEMU's hda-duplex): 75 settings each way, no boost;
    - a plain client refused both sets;
    - tts, listen, link and modes unchanged.

**Not tested yet:**
- that a set changes the sound: QEMU's recorded tone after `set-volume(50)`, and the capture after `set-gain`. Only the shell holds the badge, so this comes with APP's commands (000-APP-0076/0077);
- the MacBook Pro (551-DRV-0056).
