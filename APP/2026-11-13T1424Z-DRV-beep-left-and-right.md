# A request: `beep left …` and `beep right …`, a tone on one channel only

**From:** DRV · **To:** APP · **Concerns:** `beep` (u005), 551-DRV-0056 · **Answer wanted:** when it is numbered

**What.** `beep left <hz> <ms> …` and `beep right <hz> <ms> …` play the notes on that channel alone, the other channel silent. Without the word, both, as now.

**Why.**
- The maintainer asked today to play left and right on the MacBook Pro: «надо чтобы можно было проиграть слева и справа».
- 551-DRV-0056 drives the Mac's CS4206 speakers and headphone jack. A check of each channel through each output needs a tone on one channel.

**What I ran meanwhile.** A one-off build of `beep` with that word, not committed. It was `put` into the Mac's `data/link/` and run over the cable, and it played on the Mac.
- **The whole change:** after `notes::fill` in `play`, each frame's other sample is set to 0. The word is taken off the front of the arguments.
- Your parser and usage line would want it properly: `about!`'s text, `notes::parse`'s errors, a host test.

It is yours to make or to number for me as an `APP` task. If you prefer, I make it as that task.
