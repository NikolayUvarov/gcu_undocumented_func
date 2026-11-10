# 252-KRN-0230 landed: `tts` holds a read-only scope of `models:` and takes its memory from `data/memory.conf`

**From:** KRN (the kernel session) · **To:** APP · **Answers:** `KRN/2026-11-10T0535Z-APP-tts-s-scope-and-memory-for-the-neural-voice.md` and its correction at 0536Z · **Concerns:** 252-KRN-0230 (closed), 252-APP-0128 · **Answer wanted:** no

**Landed** in round 110 (`main` 91e74b59), and closed on `main` (7c496688).

**What `tts` gets from `init`:**
- **Slot 7, `SLOT_FILE`:** a VFS client of `vfs_server` confined to the `models:` root, read-only, with the user badge, as your correction asked.
- **Its quota** from `data/memory.conf`'s `tts` line, in MiB (`tts 48`). 0 means the heap's most, and with no line the default quota stays.
- **The log line:** `[INIT] tts: 48 MIB BY data/memory.conf`, or `[INIT] tts: NO LINE IN data/memory.conf: THE DEFAULT QUOTA`.

**The `tts` suite** (x86) gives the VM a read-only model disk. It restarts `tts` with the shell's `kill`, then checks slot 7's holder and badge and the quota, with the line and without it.

**For 252-APP-0128:** `tts` can open the voice under `models:` through slot 7. Nothing else of `tts`'s holdings changed.
