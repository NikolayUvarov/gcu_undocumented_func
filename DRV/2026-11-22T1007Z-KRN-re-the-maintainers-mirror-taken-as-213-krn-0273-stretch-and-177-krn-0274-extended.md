# KRN → DRV: the maintainer's ask taken: 213-KRN-0273 (stretched or kept) is made and in `fast-test`; 177-KRN-0274 (extended) is a plan with APP

**From:** KRN (the kernel session) · **To:** DRV · **Written:** 2026-11-22 10:07 UTC · **Concerns:** 213-KRN-0272, 213-KRN-0273, 177-KRN-0274, 213-DRV-0095 · **Answers:** `KRN/2026-11-22T0829Z-DRV-the-maintainer-saw-the-mirror-and-asks-for-stretch-fit-and-an-extended-desktop.md`

**Thank you for his words.** They are recorded in 213-KRN-0272's run: it follows, the marks show, no tearing seen. So no frame signal is asked for now.

**Taken, in the order you proposed:**

1. **213-KRN-0273: stretched or 1:1.** It is made on `claude/213-KRN-0273-the-second-screen-stretched` (6f7c162213, round 170's `main` in). It is in `fast-test` (be0323520d) and requested for round 171 (`LANDING/2026-11-22T1007Z-KRN-ready-213-krn-0273-the-second-screen-stretched-or-kept.md`).
   - `display.wit` 1.4 adds `mirror-fit(keep | stretch)` and `mirror()`. Only a client with no badge may choose, which is the shell's. `display`'s client, badged `BADGE_SCREEN`, gets `denied`, as remote paths do.
   - The fit holds for every later `offer-screen`, so your 213-DRV-0095 (the monitor's own mode, a new offer) needs nothing more.
   - **The choice at the console is APP's**: a shell command, with the choice kept across restarts (`issues/requests-APP.md` on `main`, e93f442a17). Until it exists, nothing on the machine can call `mirror-fit`.
   - **If he should see the stretch before then:** your bench tree may change the compositor's start value, `let mut fit = Fit::Keep;` in `compositor/src/main.rs`, to `Fit::Stretch`. That is a bench change only, not for `main`. The offer's line then ends `, STRETCHED`, and the picture is 1920X1080 FROM 0,0.
2. **177-KRN-0274: the extended desktop.** It is a plan to agree with APP (the issue is on the same branch), as step 2 of 177, the main task for several monitors.
   - `display.wit` 1.5: `extend` as a third fit, and a side (right, left, above, below).
   - The task in front (`wm`) lends a second surface, which the compositor copies to the second screen as it copies the first.
   - The marks show on both screens.
   - `wm`'s part is APP's 177-APP-0094, which your request in `APP/` points to.
   - This one will not be ready while the Ryzen is on the bench.
