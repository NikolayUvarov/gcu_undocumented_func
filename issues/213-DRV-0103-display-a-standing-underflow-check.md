# 213-DRV-0103 — `display`: a standing underflow check after the offer

**Type:** driver (`display`) · **Owner:** `DRV` · **Priority:** P2 · **Status:** open (2026-11-22) · **Blocked by:** — · **Main task:** [213](213-amd-ryzen-pc.md) · **Roadmap:** the drivers track · **Constitution:** MC-12.1, MC-12.2, MC-12.3

## Problem

Stage 15's watch (213-DRV-0092) reads both pipes' underflow statuses at each of 300 frames after the pattern goes off,
then nobody looks again. So it checks the first 5 s of the second picture and nothing after. It is also 5 s of the
6.2 s the production order takes to offer the second screen (213-DRV-0098, boots 84 to 89).
- **When the one underflow seen came:** within 41 frames (boot 74, before e20a0244f).
- **What could bring one later, and is not looked for:**
  - DRAM's states under the watermarks the firmware set for one display;
  - the compositor's load as the desktop changes.
- **The watch's worst moment** in boots 84 to 89 was another task's burst of a known size: 130 us at `init`'s end of
  boot. A standing check has to live with that too.

## Plan

- **The check:** once a second after the offer, while stage 15's picture is up, read four registers against the values
  stage 9 took:
  - pipe 1's `HUBP1_DCHUBP_CNTL` (`HUBP_UNDERFLOW_STATUS`, `HUBP_TIMEOUT_STATUS`) and `ODM1_OPTC_INPUT_GLOBAL_CONTROL`
    (`OPTC_UNDERFLOW_OCCURRED_STATUS`);
  - the firmware's `HUBP0` and `ODM0` likewise.
- **A new status on either pipe** gets the watch's own undo:
  1. stage 15's writes are put back at once: the pattern on, then HUBP1 blanked as stage 13 left it;
  2. `screen_gone`;
  3. stages 14 to 1 are undone;
  4. the mark stays, so nothing is tried again this boot.

  One line says so, with the status, its register and the time since the offer.
- **Nothing found:** nothing is logged, but at most every 10 minutes one line gives the checks made and the longest time
  between two. A check that silently stopped would show.
- **The switches:**
  - `STANDING_CHECK` (committed false) needs stages in place (15 or 16) and works with `HOTPLUG` on or off: in 0096's
    50 ms loop every second, or in a loop of its own.
  - The watch's length becomes a constant, `light::WATCH_FRAMES`, 300.
  - `SHORT_WATCH` (committed false) shortens it to `light::SHORT_WATCH_FRAMES`, 60, only together with
    `STANDING_CHECK`: a build that sets it without the check does not compile. The owner session decides the bench's
    value after the first runs.

## Latched or not

The check counts on the statuses being latched, and they are:
- **What the register map shows** (`dcn_2_1_0_sh_mask.h`, Linux v7.0):
  - `OPTC_UNDERFLOW_OCCURRED_STATUS` (bit 10) has its own `OPTC_UNDERFLOW_CLEAR` (bit 12), and the live state is a
    separate field, `OPTC_UNDERFLOW_OCCURRED_CURRENT` (bit 13);
  - `HUBP_UNDERFLOW_STATUS` (bits 28 to 30) has `HUBP_UNDERFLOW_CLEAR` (bit 31);
  - `HUBP_TIMEOUT_STATUS` (bits 20 to 23) has `HUBP_TIMEOUT_STATUS_CLEAR` (bit 26).
- **What Display Core does:** it reads them and then clears them (`dcn10_did_underflow_occur`, `hubp2_clear_underflow`,
  `optc1_clear_optc_underflow`).
- **What the hardware showed:** on boot 74, HUBP1's underflow and timeout statuses were read once, 676 ms after the
  frames they came in, and were still set. No request was outstanding by then.
- **Who clears them:** only stage 15's watch clears pipe 1's, and nothing in MIND Core clears the firmware's. So an
  underflow since stage 9 shows at the next check, however short it was.

## Progress (2026-11-22)

**Built** on 213-DRV-0096's head (branch `claude/213-DRV-0103-display-a-standing-underflow-check`):
- `light::standing_check` and `light::standing_undo`, and `light::Standing`, the account of checks;
- the journal keeps stage 9's four statuses for the check;
- `Light::with_watch`, `SHORT_WATCH_FRAMES`;
- in main.rs, the check in the follow loop, the switches and the build's refusal.

**Host cases** (`display_light_host.rs`, 6 new; MC-12.2: a model, not the hardware). The model latches a status until
its clear bit, as the hardware does, and gives the ODM's live bit a window of its own:
- an hour of checks finds nothing, writes nothing and says so 6 times, `600 CHECKS IN 10 MIN, THE LONGEST 1000 MS`. A
  check kept back 3 s shows as `THE LONGEST 3000 MS`.
- HUBP1's underflow, 10.4 s after the offer:
  - it is caught at the 11th check;
  - the undo comes in order: the pattern on and HUBP1 blanked, then `screen_gone`, then 14 to 1;
  - every register is as before step 3 but the mark, which stays, so a start again runs no stage.
- the firmware's HUBP0 underflow, and HUBP1's request timeout, are caught alike.
- an ODM underflow of 100 ms between two checks is caught by its latched status, though the live bit is back to 0 at
  the check.
- an underflow one frame after the watch's last is unseen by the watch and caught at the first check, with the watch of
  300 and with that of 60.
- the watch is 300 frames unless the build gives 60, never longer than 300, and main.rs's refusal is in place.

**Not shown:** anything of it on the hardware, and the cost of four reads a second there.

## Acceptance criteria

- [x] Host cases: nothing for an hour; an underflow after the offer caught within a second and undone as the watch does,
  the mark kept; the firmware's pipe and a request timeout alike; a transient that is over within the second still
  counted by its latched status; an underflow one frame after the watch ends caught within a second; the watch 300
  unless 60 is given with the check.
- [ ] On the Ryzen: the check running after the offer, its ten-minute line seen, nothing found, with the watch of 300;
  then the owner session's choice of the bench's watch.

## Related

[213-DRV-0098](213-DRV-0098-display-both-screens-at-start-in-seconds.md) (the watch's numbers, the gap at `init`'s end
of boot), [213-DRV-0096](213-DRV-0096-display-the-second-output-plugged-and-unplugged.md) (the loop it rides on),
[213-DRV-0092](213-DRV-0092-display-mind-cores-picture-on-the-second-output.md) (the watch, boot 74's underflow).
