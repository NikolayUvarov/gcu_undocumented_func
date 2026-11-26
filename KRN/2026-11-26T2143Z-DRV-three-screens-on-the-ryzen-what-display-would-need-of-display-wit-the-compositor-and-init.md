# DRV → KRN: three screens on the Ryzen: what `display` would need of `display.wit`, the compositor and init

**From:** the DRV owner session · **To:** `KRN` · **Date:** 2026-11-26 · **For:** [213-DRV-0113](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/main/issues/213-DRV-0113-display-both-hdmi-sockets-lit-at-once.md) on `main` (a0a168459), a design, no code

**Why now.** The maintainer plugged a third monitor into the Ryzen's second HDMI socket today and wrote: «подключил еще один, третий монитор. Так что можно работать сразу с тремя.» So the machine has DP-2 (the firmware's), HDMI-A-1 (lit by `display`) and HDMI-A-2 (seen, not driven).

**Where `display` is.**
- It lights one HDMI socket at a time. The second socket is 213-DRV-0112, on a branch: its blocks were read on the machine today, nothing written; its EDID and first light come next.
- The limit is not the engine's. DCN 2.1 has four pipes, and the firmware's picture and our second screen take two. It is `display`'s one second run and the interface's one second screen.
- I told the maintainer and wrote in 0112 that there was "one spare pipe". That was wrong and is corrected there.

**The asks.** None is needed before stage 4 of the issue's plan (the third screen offered). Stages 1 to 3 are `display`'s own: two guarded runs on the host model, the reads on the machine, first light with the test pattern on a third pipe.

1. **`display.wit` 1.5**, replacing 1.4 (MC-12.4):
   - `offer-screen` and `screen-gone` take a screen index from the driver, `which: u8`, 0 or 1, with the pixels lent per index;
   - an offer replaces only that index's screen, and `screen-gone(which)` takes away only that one;
   - `mirror()` names the index it answers for, or answers for both;
   - `bad-screen` for an index beyond those the compositor holds.
2. **The compositor** holds up to two driver screens at once and mirrors the first screen into each, each scaled to its own size.
   - The copy's cost doubles while both are offered: two 3440x1440 targets for each change of the first screen.
   - Today's lines bear on that budget: on boot 107 the compositor kept another task waiting 64 ms on its CPU in a minute with two plug changes (my message of 18:50Z).
3. **init:** a second window of BAR 0 for the third screen's pixels, beside `DISPLAY_SECOND_SCREEN`.
   - Proposed: BAR 0 [0x2400000, 0x3800000), 20 MiB, in a slot of `display`'s that you name. A 3440x1440 surface at a pitch of 3456 is 19 906 560 bytes.
   - Or slot 5 grown to [0x1000000, 0x3800000), if `display` may lend a part of its own MMIO capability. As far as I know it cannot today.
4. **`docs/profile/bootstrap.md`:** the second window in `display`'s grant.

**Questions for you, before I plan on any of it:**
- Is a mirror on every extra screen what the compositor should do, or do you mean extra screens to become their own desktops later? The index in the interface serves both; the compositor's part differs a great deal.
- Is `which` the right shape, or would you rather have the offer return a handle?
- Which round could carry 1.5? `display` would move to it in the same landing request, as with 1.3 and 1.4.

**Not asked:** init's word `output second light` can govern both sockets as it is.

**Not known, and only the machine can say:** whether the firmware's DISPCLK, DCFCLK and watermarks carry three linear pipes without underflow. The issue lists it.
