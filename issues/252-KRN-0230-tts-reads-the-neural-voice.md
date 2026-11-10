# 252-KRN-0230 — `tts` reads the neural voice: a read-only scope of `models:`, and its memory by `data/memory.conf`

**Type:** `init` (grants, quota) · **Owner:** `KRN` · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** 252 (APP's), for [252-APP-0128](252-APP-0128-tts-speaks-with-the-neural-voice.md) · **Constitution:** MC-3.1, MC-4.2

Numbered from APP's request of 2026-11-10 in `requests-KRN.md`.

**Recorded by:** the tools track (`APP`), 2026-11-10, for [252-APP-0128](252-APP-0128-tts-speaks-with-the-neural-voice.md). Asked in `ALL/2026-11-09T1852Z-APP-tts-with-the-neural-voice-which-way.md` (with NRL's way through `nnd` beside it).

## Problem

- **Why the neural voice** ([252-APP-0041](252-APP-0041-russian-speech-on-the-macbook.md)): the formant voice is understood at CER 40.5 % on the 24 Russian sentences, and the neural voice (Vosk TTS 0.7, which `speak` runs through `mind::nn`) at 0.80–1.75 %.
- **What `tts` holds today:** its endpoint and a client of `audio_gw`. It can read no model, and its quota does not hold the voice: 128 MB of network and a 13 MB dictionary; `speak` asks for 320 MiB.

## Plan

As `init` gives `nnd` its models and its memory (262-KRN-0178), for `tts`:
- **A read-only scope of `models:`** in `SLOT_FILE`, never made, as `nnd`'s second scope.
  - Not of `tts-ru-vosk-0.7/` alone: `mind::model_file::read` takes a file only if its SHA-256 is the one `models:MANIFEST.json` lists (MC-4.2), and the manifest is at the disk's root.
  - Without the model disk there is none, and `tts` speaks with the formant voice as now.
- **Its quota from a line `tts <MiB>` of `data/memory.conf`,** read at each start, the owner's choice. Without a line, the default as now, and the formant voice.
- **Nothing else:** no network, no workers, no other volume.

The tools track's part (252-APP-0128):
- `tts` loads the voice at its first Russian phrase when both are there, and speaks with it;
- the formant voice for English, and as the fallback;
- the log says once which voice speaks and why.

## Acceptance criteria

- `tts` holds the scope only with the model disk, and the quota only with the line; `docs/profile/bootstrap.md`'s row says so.

## Related

[252-APP-0128](252-APP-0128-tts-speaks-with-the-neural-voice.md), [262-KRN-0178](262-KRN-0178-nnd-as-a-boot-service.md) (the same for `nnd`).
