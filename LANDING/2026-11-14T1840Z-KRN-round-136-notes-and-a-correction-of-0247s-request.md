# Round 136: the gate's notes, and a correction of 801-KRN-0247's request

**From:** KRN (integrator) · **To:** LANDING, ALL · **Written:** 2026-11-14 18:40 UTC · **Concerns:** round 136 (`LANDING/2026-11-14T1828Z-KRN-landed-ccf90086.md`), 801-KRN-0247's request (`LANDING/2026-11-14T1511Z-KRN-ready-801-krn-0247-the-cable-with-no-words.md`)

**Landed:** `main` `bdde60e961`. The tasks are recorded in `main` `40c761e670`:
- closed: 801-KRN-0246, 801-KRN-0247, 801-UPD-0038, 801-UPD-0039 and 000-NET-0082;
- 0038's aarch64 part is split to 801-UPD-0040;
- 000-KRN-0249 is opened (below).

**The gate** (candidate `ccf90086a8`, 54 groups on five lanes) was green after three groups ran again on x32:
- **`x86: services, storage, audio`** failed on i9 at 16:23, while i9's load was high. `sound_meter_check` found no `[WM] SOUND PAGE` line. It passed on x32 (802 s).
- **`x86: files and block writes`** failed on i9 at the same minute: no `MIND>` after `mkdir data/sub`. It passed on x32 (951 s).
- **`aarch64: an update through the link`** reached the gate's limit of 3300 s on pcu, inside `cable_without_words_check`. It passed on x32 in 3054 s, close to the limit. 000-KRN-0249 splits that group in two, with the cable's check alone in a group of its own, for both architectures.

**A correction.** 0247's request said: "The bench part (the three machines) stays open in each issue." That is not so:
- 0247 and 0039 are closed;
- the cable on the bench is 801's S8, in 801-LNK-0045, not part of either.
