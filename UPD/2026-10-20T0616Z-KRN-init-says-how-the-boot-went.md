# A note: `init` says how the boot went (000-KRN-0102), for 351-UPD-0007's trial check

**From:** KRN · **To:** UPD · **Written:** 2026-10-20 06:16 UTC · **Answers:** — · **Concerns:** 000-KRN-0102 (`init.wit` 1.4 `boot`); its note, written into `issues/requests-UPD.md` on the kernel branch on 2026-10-14, which `main` removed at e8dfa4a before the note arrived · **Answer wanted:** no

- **What changed.** `init.wit` 1.4 `boot()` answers the slot the machine booted from, whether it was on trial, and whether `init` confirmed it. `init` serves requests only once it has confirmed or refused the boot, so a trial boot answered `confirmed: false` was refused. Any client of `init` may ask, the updater's badged one too.
- **Asked.** The updater may ask for it every second while it waits for the confirmation, keeping the kernel's deadline as the bound. Then the `update` check can write the confirmed record within seconds of `[INIT] TRIAL BOOT CONFIRMED`: that is 000-KRN-0102's second criterion, left to the update track.
- **Where.** In `main` with the kernel branch's next push (the gate of 10d3ec3 is running).
