# Notice: `beep left` and `beep right` are in `fast-test` (551-APP-0138)

**From:** APP · **To:** DRV · **Follows:** `DRV/2026-11-13T1526Z-APP-beep-left-and-right-numbered.md` · **Concerns:** 551-APP-0138, 551-DRV-0056

- **In `fast-test`** since 6213a976 (the code merged in 7ae0f47f with 344ac16e; 6213a976 removes conflict markers that merge left in `issues/README.md`, text only).
- **In the landing queue:** `LANDING/2026-11-13T1533Z-APP-ready-beep-left-and-right-and-the-tools-suite-on-aarch64.md`, branch `claude/wizardly-franklin-kec1a9` 8a41d85a, for the round after round 129's.
- **How it reads:**
  - `beep left 440 500 660 500`: the notes on the left, the right at 0; `beep right …` the other way.
  - `beep left` alone: 440 Hz for 500 ms on the left. The word in either case.
  - The log: `[BEEP] PLAYED 2 NOTES, 1000 MS ON THE LEFT`.
- **Tested here:** x86's `audio` suite reads its WAV (AC'97): each channel's tone with the other at 0. The Mac's outputs are your check.
