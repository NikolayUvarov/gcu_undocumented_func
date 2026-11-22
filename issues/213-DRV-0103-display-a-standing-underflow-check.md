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

**Boot 93 (2026-11-22, release 202611221429, the bench tree 82e458e2f with ba592c2b8; `STANDING_CHECK` true,
`SHORT_WATCH` and `HOTPLUG` false): the standing check's first 50 minutes, nothing found.** From the owner session's log
of that boot, which runs to 3164.9 s:
- **The offer:** `THE SECOND SCREEN OFFERED 6181.2 MS AFTER DISPLAY'S START`. The compositor took it at 9.553 s of the
  boot.
- **The watch:** 300 frames clean, as in boots 84 to 91, but its longest gap was 2176 us, before frame 95 (boots 84 to
  91: 129 to 130 us, at frames 27 to 32).
  - Placed by its frame, it fell at about 6.10 s of the boot. That is where the Wi-Fi driver of that image logs the end
    of its scan, `[RTW89] SCAN: 21 PASSIVE TUNES, THE LONGEST 3249 US`, at 6.113 s, and `wlan` starts to join.
  - The image carried a Wi-Fi fix. Which task held the CPU the log does not tell.
  - 2.2 ms is an eighth of a frame, so no frame passed unread.
- **The check:**
  - At 9.616 s: `THE STANDING CHECK: BOTH PIPES' UNDERFLOW STATUSES READ EVERY 1000 MS WHILE THE SECOND PICTURE IS UP;
    THE WATCH 300 FRAMES`.
  - Then five times `S15 STANDING CHECK: 600 CHECKS IN 10 MIN, THE LONGEST 1014 MS BETWEEN TWO; NOTHING NEW ON EITHER
    PIPE` (at 609.9 s, and with 1021 MS at 1210.2, 1810.4, 2410.6 and 3010.9 s). That is 3000 checks in 50 minutes, the
    longest time between two 21 ms over its second, and no underflow and no request timeout on either pipe.
  - The last 2.5 minutes to the log's end are inside the next ten, so no line is due there.
- **The window manager:** the maintainer started it at 2486.9 s (`[WINDOWS] MANAGER PID 49 ATTACHED`, `SCREEN OF PID 49
  IN FRONT`). The ten minutes that ended at 3010.9 s were checked with `wm` in front and found nothing.

**With `HOTPLUG` on** (the boot of 0096's run, 2026-11-22 from 15:40 UTC, the same `display` code): the first line read
`S15 STANDING CHECK: 585 CHECKS IN 10 MIN, THE LONGEST 1061 MS BETWEEN TWO; NOTHING NEW ON EITHER PIPE`.
- The check then rides `HOTPLUG`'s 50 ms polls and comes at the first poll past its second, up to one poll late. So
  there are fewer than 600 checks in ten minutes, and the longest time between two is a second and a poll.
- As planned.

**Boot 94** (0096's run with the maintainer, `HOTPLUG` and `STANDING_CHECK` on): two take-backs and relights inside one
ten minutes. Its line at 1210.482 s read `567 CHECKS IN 10 MIN, THE LONGEST 12270 MS BETWEEN TWO; NOTHING NEW ON EITHER
PIPE`.
- The 12.27 s was the dark span from the cable's take-back (1044.7 s) to the first check after the relight (about
  1056.4 s). The check does not run while there is no picture, and the account did not start afresh.
- So the line could no longer show a check that silently stopped, which was its purpose.
- **Since then:**
  - `Standing::dark` is called at each take-back. No time between two checks spans a dark span.
  - The line says the dark spans apart: `... THE LONGEST n MS BETWEEN TWO WHILE LIT; DARK d TIMES, s S IN ALL; NOTHING
    NEW ON EITHER PIPE`.
  - A ten minutes with no dark span keeps the line as before.
  - Host case: an unplug and a plug inside one ten minutes give the lit time's longest (1000 ms) and `DARK 1 TIMES`, and
    the ten minutes after give the old line.
  - The running bench keeps the old line until the next display change.

**A take-back that went wrong was not logged (found 2026-11-22, fixed the same day on the owner session's word).**
- **The flaw:** in the production order, `light::take_back` (0096's) held its `U` lines and never called `failed()`. A
  table that stopped in an off call, a `screen_gone` the compositor did not take, or a firmware's pipe that read CHANGED
  showed only in the final `STEP 3 TAKEN BACK IN ...` line. The build's own undo after the stages (`UNDO_AFTER_STAGES`)
  had the same flaw. Every journaled write was put back all the same.
- **Since then:**
  - `undo` says whether its stage went wrong: the off call's table stopped, or `screen_gone` was not taken.
  - Both undos run their stages through `undo_all`. A stage that went wrong has the detail held so far logged at once
    (`failed()`), and every line after it.
  - A firmware's pipe that does not read UNCHANGED logs the held detail before the last line.
  - The last line then ends `; n STAGES' UNDOS WENT WRONG, THEIR DETAIL ABOVE`. One that went right keeps its two lines,
    as on boot 94.
  - **Host cases** (production order):
    - an unplug gone right: two lines;
    - stage 8's off call stopping: its `U8 TABLE 76 STOPPED` line logged, with the detail before it and every undo after;
    - `screen_gone` not taken;
    - the firmware's pipe changed: the whole detail logged;
    - the build's own undo with stage 8's off call stopping.
  - Not shown on the hardware: no take-back there has gone wrong.

**Not shown by it:**
- the check finding anything on the hardware, and so its undo after the offer and `screen_gone` there;
- the watch of 60 (`SHORT_WATCH`);
- the check with `HOTPLUG`;
- what the four reads a second cost;
- what the maintainer saw on either screen.

This is that boot, on that machine with its two monitors (MC-12.1, 12.9).


**The short watch on the hardware** (the boot after 94, bench tree 047917b60, `SHORT_WATCH`, `STANDING_CHECK` and
`HOTPLUG` true; read over the cable at 16:14 UTC):
- **The offer:** `THE SECOND SCREEN OFFERED 2165.7 MS AFTER DISPLAY'S START`, at 5.635 s of the boot. That is 4.0 s
  sooner than with the watch of 300.
- **`STEP 3'S TIMES (MS)`:** IN ALL 2028.9; 15 1040.3; 15'S WATCH 998.3; 9'S COUNT 510.5.
- **S15:** `WATCHED 60 FRAMES IN 998 MS CLEAN, LONGEST GAP 26 US AT FRAME 53`.
- **The start lines:** HPD 1 followed, and the standing check's start line naming `THE WATCH 60 FRAMES`.

## A proposal: the offer before the watch (2026-11-22; not built, the owner session's decision)

**Where the time goes.** What the maintainer saw at boot 94 as «через некоторое время появилось изображение заставки», then
the screen, is stage 10's picture shown through stage 15's watch before stage 16's offer: 5 s with the watch of 300,
1 s with that of 60. Of the 2.17 s to the offer with the short watch:
- the watch takes 1.0 s;
- stage 9's count, 0.51 s;
- the waits of stages 5, 13 and 15, 78 ms;
- the 20 ms pace of about 20 lines, some 0.4 s;
- step 2, 0.11 s.

**The proposal: stage 16's offer right after stage 15's pattern goes off, then the watch.**
- **Unchanged:** stage 15's first check (three frames behind the pattern, and no new underflow) still comes before the
  pattern goes off, and before the offer.
- **On any finding of the watch,** the standing check's undo, which this task has already built and tested:
  1. stage 15's writes put back at once (the pattern on, HUBP1 blanked);
  2. `screen_gone`;
  3. stages 14 to 1.
- **The offer refused:** the watch runs all the same, and the stop comes after it, as stage 16 not ready does now.
- **What it gains:** the mirror about 1 s sooner with the short watch (the offer at about 1.2 s after `display`'s
  start), or 5 s with the watch of 300.
- **What it costs in safety:**
  - **A failed watch then shows the desktop on HDMI-A-1 for up to a second before the screen goes dark,** where now it
    shows stage 10's picture. The compositor gets `screen_gone` within that second. That path ran twice on boot 94 (the
    take-backs), but never after an underflow.
  - **The compositor's first full copy into the surface (19.9 MB at 3440x1440) falls inside the watch,** not after it.
    That is load the second screen will carry anyway. Watching under it tests more of the real use, not less, but it is
    a change in what the watch has shown on boots 75 to 94.
  - **The order of the first undo writes and `screen_gone` stays:** the pipe is blanked first.
  - **No check is weakened:** the frames before the pattern goes off, the per-frame watch and the standing check after
    it all stay.
- **What it would need:**
  - stage 16 moved before the watch in the plan, so that the watch is stage 16's last step or a stage 17;
  - the watch's failure path ending with `screen_gone`, with host cases for:
    - an underflow during the watch after the offer;
    - an offer refused before a clean watch;
    - one refused before a failing watch.
- **Not proposed:**
  - dropping the watch for the standing check alone. The check reads once a second, so an underflow on DP-2's pipe would
    stand up to a second before the undo, against one frame with the watch.
  - a shorter count in stage 9 (decided: 30).
  - a lower line pace. That is the log's rule, not `display`'s.

## For a successor (state on 2026-11-22, 16:45 UTC)

This replaces the section of the same name in 213-DRV-0095, which describes the state before 0098.

**The branches,** DRV's:

| Branch | Head | What | Where it is |
|---|---|---|---|
| `claude/213-DRV-0095-display-the-monitors-own-mode` | 1e148b81f | the EDID's mode (0095), on 0092's | in `main` since round 172 (7346ab3d5, 2026-11-22 15:19 UTC) |
| `claude/213-DRV-0098-display-both-screens-in-seconds` | 598045d8f | the production order (code e458838ac, a71a47b83; the rest text) | landing request posted; waits for the next round |
| `claude/213-DRV-0096-display-the-second-output-plugged-and-unplugged` | 8823e6433 | `HOTPLUG` (code 5e63d41b4, on a71a47b83; the rest text); both criteria met on boot 94 | landing paragraph given; after 0098 |
| `claude/213-DRV-0103-display-a-standing-underflow-check` | this branch's head | `STANDING_CHECK`, `SHORT_WATCH` (code ba592c2b8, then 9acc0b773's dark spans and the take-back's detail, above) | after 0096 |

**The bench:** the release on the Ryzen since about 16:10 UTC runs a5c933530's `display` with `HOTPLUG`,
`STANDING_CHECK` and `SHORT_WATCH` true. Its standing check's line is the one before 9acc0b773, without dark spans, and
its take-back holds a failure's detail.

**LEG9 restarted at 15:21 UTC** and wiped `/tmp`: the worktrees and the scratch scripts. Nothing pushed was lost.
- **To work on a branch again:** `git -C <main checkout> worktree add --detach <path> origin/<branch>`, commit there,
  then `git push origin HEAD:refs/heads/<branch>`.

**The switches in `display/src/main.rs`.** All are committed off, so the committed `display` maps, reads and writes
nothing.
- **From 0086 to 0095** (0095's section):
  - `BATCHES_ALLOWED` (11 for all) and `STAGES_ALLOWED` (16 for all);
  - `EDID_ALLOWED` (needs 11 batches), `HDMI_SIGNALLING`, `MODE_FROM_EDID` (needs `EDID_ALLOWED`);
  - `UNDO_AFTER_STAGES`.
- **`SURVEY_ORDER`** (0098): true gives the survey's order of boots 70 to 81; false, the committed value, gives the
  production order.
- **`HOTPLUG`** (0096): HPD 1 followed after the stages. It needs `STAGES_ALLOWED` above 0 and not `UNDO_AFTER_STAGES`.
- **`STANDING_CHECK`** (this task) needs stage 15 or 16 in place and not `UNDO_AFTER_STAGES`. With `HOTPLUG` it rides
  the 50 ms polls (about 585 checks in ten minutes), without it a loop of its own (600).
- **`SHORT_WATCH`** (this task): the watch of 60 frames, only with `STANDING_CHECK`. The build refuses it otherwise.

**The host cases:**
- **The suites:** `tests/display_{host,atom,batches,ddc,hpd,light,picture,screen}_host.rs`, 113 cases on this
  branch's code.
- **With the Ryzen's tables:** with `MIND_AMD_VBIOS` naming the Ryzen's VBIOS image, they also run the Ryzen's own
  tables.
  - The image must never enter the repository, a commit, an issue or a message.
  - The owner session has the ACPI VFCT table outside the repository. The image is inside it: offset 52 names its
    header, the header's bytes 24 to 27 its length, and the image follows the header's 28 bytes, as `atom::vfct_image`
    reads it. That table is what the Ryzen's `init` copies to `log:acpi/` at every boot.
- **The full suite:** `scripts/host_tests.sh` runs them all. On LEG9 its ssh tests need
  `MIND_SSHD=/tmp/claude-1000/sshd-pkg/root/usr/sbin/sshd`, which the owner session puts back after a restart. Heavy
  runs take `/tmp/claude-1000/heavy.lock` with `flock`.

**The hardware evidence:**
- **The survey's order:** boots 70 to 81 (0092, 0095).
- **The production order:** boots 84 to 89 and 91, 6.16 to 6.18 s to the offer (0098's table).
- **The standing check:** boot 93, 50 minutes, nothing found.
- **0096's run with the maintainer:** boot 94 (0096's issue).
- **The short watch:** the boot after 94, the offer at 2165.7 ms.
- **The logs:** the owner session keeps boot 93's, boot 94's to 1168 s and the short-watch boot's cable read under its
  handoff directory. Its copies of boots 84 to 91 went with the restart; the boots' own logs stay on the Ryzen's log
  volume.

**What waits:**
- **For the maintainer:**
  - his decision on 0099's proposal;
  - 0097's cold path, which is his decision;
  - his look at the 3440x1440 picture (0095's criterion 4).
- **For the owner session:**
  - the decision on the proposal above (the offer before the watch);
  - closing 0098 and 0096 once they have landed.

## Acceptance criteria

- [x] Host cases: nothing for an hour; an underflow after the offer caught within a second and undone as the watch does,
  the mark kept; the firmware's pipe and a request timeout alike; a transient that is over within the second still
  counted by its latched status; an underflow one frame after the watch ends caught within a second; the watch 300
  unless 60 is given with the check.
- [x] On the Ryzen: the check running after the offer, its ten-minute line seen, nothing found, with the watch of 300;
  then the owner session's choice of the bench's watch. Boot 93: 50 minutes, five lines, nothing found, the watch of 300.
  The owner session chose the watch of 60 for the bench, with the check, at 16:10 UTC. The boot after 94 offered at
  2165.7 ms, its 60 frames clean. That machine with its two monitors (MC-12.1).

## Related

[213-DRV-0098](213-DRV-0098-display-both-screens-at-start-in-seconds.md) (the watch's numbers, the gap at `init`'s end
of boot), [213-DRV-0096](213-DRV-0096-display-the-second-output-plugged-and-unplugged.md) (the loop it rides on),
[213-DRV-0092](213-DRV-0092-display-mind-cores-picture-on-the-second-output.md) (the watch, boot 74's underflow).
