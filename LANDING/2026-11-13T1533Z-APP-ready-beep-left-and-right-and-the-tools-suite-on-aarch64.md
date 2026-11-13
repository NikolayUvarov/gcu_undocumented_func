# Ready: `beep left` and `beep right` (551-APP-0138), with the `tools` suite on aarch64 (000-APP-0137)

**Ready:** `claude/wizardly-franklin-kec1a9` 8a41d85ac5ec78aa13a98294cf551a6a56463513

It contains `main` d8c0ec8225 and every commit of `LANDING/2026-11-13T1220Z-APP-ready-the-tools-suite-on-aarch64-and-hws-names.md` (21f3a365), which it **replaces**: that one was not answered and is in no candidate. `LANDING/2026-11-13T0941Z-APP-ready-again-run-d-read-only-the-clipboard-closed-and-three-issues.md` (171d37ec), in the candidate of round 129, stays as it is; this one is for the round after it.

- **Closes** 551-APP-0138 (asked by DRV for 551-DRV-0056) and 000-APP-0137 (as in the 1220Z request).
- **`beep`:** `beep left <hz> <ms> …` and `beep right …` play the notes on that channel, the other's samples 0; the word alone sounds 440 Hz for 500 ms; without it, both as before. The log line names the channel.
- **Tested** in this container, x86, 4 CPUs: the `audio` suite (its WAV holds the left's 660 Hz with the right at 0, then the right's 440 Hz with the left at 0) and `hda`; host tests `beep_host` (4). The 1220Z part as it said there (`tools` on aarch64 and x86, 11 each; `monitor_host`).
- **Merges** with every request waiting (KRN's 0950Z b65e1d34, UPD's 1015Z 32d0486c, STO's 1320Z faf2910c) without a conflict.
