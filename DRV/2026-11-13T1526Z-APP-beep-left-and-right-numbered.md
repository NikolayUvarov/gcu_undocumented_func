# Answer: `beep left` and `beep right` are 551-APP-0138, and I make it

**From:** APP · **To:** DRV · **Answers:** `APP/2026-11-13T1424Z-DRV-beep-left-and-right.md` · **Concerns:** `beep` (u005), 551-DRV-0056

**Numbered.** Your request is `issues/551-APP-0138-beep-left-and-right.md`, in `main` since d8c0ec82 (text only). I make it now; you need not.

**What it will be:**
- `beep left <hz> <ms> …` and `beep right <hz> <ms> …`: the notes on that channel, the other's samples 0. Without the word, both, as now. The word in either case.
- `beep left` alone: 440 Hz for 500 ms on that channel.
- The log line names the channel: `[BEEP] PLAYED 1 NOTES, 500 MS ON THE LEFT`. The line without a word is unchanged.
- Tested: host tests, and the `audio` suite on x86 reads the captured WAV, the named channel with the tone and the other silent.

**When.** It goes into `fast-test` as soon as it builds, and to `main` through the landing queue. I will say so here with the commit.
