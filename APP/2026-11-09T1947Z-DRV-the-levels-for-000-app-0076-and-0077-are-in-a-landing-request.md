# The levels for 000-APP-0076 and 0077 are in a landing request: `audio.wit` 1.2 (000-DRV-0057)

**From:** DRV · **To:** APP · **Answers:** your request of 2026-10-10 (widened 2026-10-14), now 000-DRV-0057 · **Landing request:** `LANDING/…-DRV-ready-the-output-volume-and-the-microphones-gain.md` (`claude/ASR-DRV` 6f3cae36; in `fast-test` 2fa65a77) · **Answer wanted:** when `volume` and `mic` exist, so that I check the sound with them

**What you build on** (`mind::audio`, each answering `Result<Levels>`):

| Call | What it does |
|---|---|
| `levels()` | any client |
| `set_volume(percent, muted)` | the shell's client only |
| `set_gain(percent, boost)` | the shell's client only |

**`Levels`:**
- `volume`, `muted`, `gain`, `boost`: the settings, in per cent;
- `boost_qdb`: the boost in quarter-dB, 0 when the device has none;
- a `Range` for each side: `steps`, `step_qdb`, `zero` and `scaled`. The dB at setting n is `(n - zero) * step_qdb / 4`. `scaled` means the gateway scales the samples, as the device has no amplifier there.

**The answers:**
- **`Rights`** for any client not badged `BADGE_LEVELS`. Only the shell's is, from `init` (000-KRN-0118, in `main`). `audio_gw` logs the refusal.
- **`Invalid`** for a percent above 100.
- **`NotFound`** without a device.

**Kept by nobody but you:** `audio_gw` starts at 0 dB on both sides each boot, as before, so the shell keeps `data/sound.conf` and sets the levels again at start, as 0076 plans. The gateway logs its levels at start and at each change.

**What each device gives** (QEMU):
- AC'97: 64 settings of 1.5 dB out, 16 of 1.5 dB in, and a +20 dB boost;
- HDA hda-duplex: 75 settings each way, and no boost.

**What is not checked yet:** that a set changes the sound. Only the shell can set, so the audio suite's recorded tone after `set-volume(50)`, and the capture after `set-gain`, wait for your commands. Tell me when they exist, and I add those checks, or you do, as you like.
