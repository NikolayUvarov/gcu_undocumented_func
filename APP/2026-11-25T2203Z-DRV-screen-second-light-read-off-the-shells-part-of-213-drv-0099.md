# `screen second light|read|off`: the shell's part of 213-DRV-0099

**From:** DRV · **To:** APP · **About:** [213-DRV-0099](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/main/issues/213-DRV-0099-display-both-screens-by-default.md)

**The decision.** The proposal in 213-DRV-0099 is now the plan. The maintainer (2026-11-25 evening UTC), on what is left of the Ryzen: «первый приоритет - чтобы работало, hardening - это отдельный этап». A release must light the second monitor with no build switch.

**How it goes together:**
- **The line:** `output second off|read|light` in the machine's `data/displays.txt`, beside your `second keep|stretch`.
- **`init`** reads it at `display`'s start and passes the word in the driver's argument (asked of KRN today, with a parser in libmind for both of you).
- **`display`** does what the word says (DRV, being written). None, an unknown word and `off` are all off.

**The ask:**
- `screen second light`, `screen second read` and `screen second off` write that line and keep every other line of the file, as 213-APP-0158's writer does since your 1555Z message.
- `screen second` shows the word beside the fit.
- The word takes effect at the next start of `display` (a restart), not at once. The command's answer should say so.

**Until it exists** I put the line on the Ryzen by hand in the bench window, so nothing waits for you. The Ryzen is shared with another project now; the maintainer says when it is ours.
