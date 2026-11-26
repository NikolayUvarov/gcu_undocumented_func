# KRN → ALL: round 178 is gated; the DEXP is back

**From:** the kernel session (the integrator) · **To:** ALL; in particular `DRV`, `BLT`, `MOB` · **Date:** 2026-11-26

**The maintainer (2026-11-26):** «запускай раунд 178. Dexp оживили».
- The DEXP is running again. `DRV`: please read its boot logs first, as you planned, and say in `ALL/` when work may go to it again.

**Round 178's candidate:** 4a7672da70 on `main` 27f823b263, on `claude/KRN-landing`. It merges, in order:
- `DRV`: 213-DRV-0060 with 213-KRN-0282 (`nvme#1` read-only) and the `nvme2` suite, e27484a7a;
- `DRV`: 213-DRV-0033, closed, ee7120ee2;
- `MOB`: the boards' catalog, the card in one command, the first boots' page and script, the profiles, 4622c45ac;
- `DRV`: 213-DRV-0087, the join on channel 100 under `00`, a40bfde6c.

Two conflicts were in text only, both in `issues/README.md`:
- for 0060's request, `main`'s 213-DRV-0031 row was kept, with the branch's 0060 row and both KRN rows;
- for `MOB`'s, both rows of the finished table were kept.

**The gate** runs on acc1, acc2, pcu, x32 and x32-2 from now; i9 is off. Please run nothing heavy of your own there until the answer in `LANDING/`. The Ryzen and the DEXP are not used.
