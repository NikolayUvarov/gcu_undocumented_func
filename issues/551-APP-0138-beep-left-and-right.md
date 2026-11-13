# 551-APP-0138 — `beep left` and `beep right`: a tone on one channel

**Type:** tools (`beep`) · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [551](551-sound-on-pcs.md) · **Roadmap:** track A · **Constitution:** MC-6.1, MC-12.1

**Asked by:** DRV, for [551-DRV-0056](551-DRV-0056-the-macbook-pros-jack-and-internal-microphone.md), in a message of 2026-11-13 (`APP/2026-11-13T1424Z-DRV-beep-left-and-right.md` on `internal-dev-messages`). Numbered on 2026-11-13.

## Problem

`beep` ([u005](../issues-done/u005-beep-without-a-screen.done)) plays its notes on both channels. A check of each channel through each output of the MacBook Pro's codec (the speakers, the headphone jack) needs a tone on one channel and silence on the other; the maintainer asked to play left and right.

## Plan

- **A channel word first:** `beep left <hz> <ms> …` and `beep right <hz> <ms> …` play the notes on that channel, the other's samples 0. Without the word, both, as now.
- **The word alone:** `beep left` sounds one note, 440 Hz for 500 ms, on that channel.
- **In `beep/src/notes.rs`:** the word taken off the front of the arguments, and `fill` writes one channel or both; `about!`'s text names the word, and a word that is not a number elsewhere is refused as now.
- **Logged:** `[BEEP] PLAYED 1 NOTES, 500 MS ON THE LEFT`; the line without a word is unchanged.

## Acceptance criteria

- **Host tests (`tests/beep_host.rs`):** the word read, alone and before notes, in either case; a word after a number refused; `fill` leaves the other channel at 0.
- **QEMU (`audio` suite, x86):** `beep left` and `beep right` in the captured WAV: the named channel holds the tone, the other is silent.

## Related

[551](551-sound-on-pcs.md), [551-DRV-0056](551-DRV-0056-the-macbook-pros-jack-and-internal-microphone.md), [u005](../issues-done/u005-beep-without-a-screen.done).
