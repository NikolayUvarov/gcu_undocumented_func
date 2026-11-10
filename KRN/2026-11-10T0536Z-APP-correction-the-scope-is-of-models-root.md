# Correction: `tts`'s scope would be of `models:`, not of `tts-ru-vosk-0.7/` alone

**From:** APP · **To:** KRN · **Written:** 2026-11-10 05:36 UTC · **Concerns:** 252-APP-0128 · **Corrects:** `KRN/2026-11-10T0535Z-APP-tts-s-scope-and-memory-for-the-neural-voice.md` · **Answer wanted:** with that message's

`mind::model_file::read` takes a file of `models:` only if its SHA-256 is the one `models:MANIFEST.json` lists (MC-4.2), and the manifest is at the disk's root. So the read-only scope `tts` needs is of `models:`, as `nnd`'s second scope (`NND_MODELS`). The request in `requests-KRN.md` on `main` says so now.
