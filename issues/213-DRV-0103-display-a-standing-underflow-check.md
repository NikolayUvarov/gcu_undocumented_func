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

**Not shown by it:**
- the check finding anything on the hardware, and so its undo after the offer and `screen_gone` there;
- the watch of 60 (`SHORT_WATCH`);
- the check with `HOTPLUG`;
- what the four reads a second cost;
- what the maintainer saw on either screen.

This is that boot, on that machine with its two monitors (MC-12.1, 12.9).


## For a successor (state on 2026-11-22, 15:45 UTC)

This replaces the section of the same name in 213-DRV-0095, which describes the state before 0098.

**The branches,** DRV's:

| Branch | Head | What | Where it is |
|---|---|---|---|
| `claude/213-DRV-0095-display-the-monitors-own-mode` | 1e148b81f | the EDID's mode (0095), on 0092's | in `main` since round 172 (7346ab3d5, 2026-11-22 15:19 UTC) |
| `claude/213-DRV-0098-display-both-screens-in-seconds` | 598045d8f | the production order (code e458838ac, a71a47b83; the rest text) | landing request posted for 598045d8f; waits for the next round |
| `claude/213-DRV-0096-display-the-second-output-plugged-and-unplugged` | d5585699d | `HOTPLUG` (code 5e63d41b4, on a71a47b83; then text) | after 0098 and its hardware run with the maintainer |
| `claude/213-DRV-0103-display-a-standing-underflow-check` | this branch's head | `STANDING_CHECK`, `SHORT_WATCH` (code ba592c2b8, on 5e63d41b4) | after 0096; boot 93 is its first run |

LEG9 restarted at 15:21 UTC and wiped everything under `/tmp`: the worktrees and the scratch scripts. Nothing that had
been pushed was lost.
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
- **`HOTPLUG`** (0096): HPD 1 followed after the stages.
  - It needs `STAGES_ALLOWED` above 0 and not `UNDO_AFTER_STAGES`.
  - It is not on the bench until the maintainer is at the monitor. What HPD 1 does in the monitor's standby is not
    known (0096, "A monitor in standby").
- **`STANDING_CHECK`** (this task) needs stage 15 or 16 in place and not `UNDO_AFTER_STAGES`. It works with `HOTPLUG`
  on or off.
- **`SHORT_WATCH`** (this task): the watch of 60 frames, only with `STANDING_CHECK`. The build refuses it otherwise.
- **The bench of boots 84 to 91:**
  - `BATCHES_ALLOWED` 11 and `STAGES_ALLOWED` 16;
  - `EDID_ALLOWED`, `HDMI_SIGNALLING` and `MODE_FROM_EDID` true;
  - `UNDO_AFTER_STAGES` and `SURVEY_ORDER` false.

  Boot 93's release added `STANDING_CHECK` true, with `SHORT_WATCH` and `HOTPLUG` false.

**The host cases:**
- **The suites:** `tests/display_{host,atom,batches,ddc,hpd,light,picture,screen}_host.rs`, 110 cases on this
  branch's code.
- **With the Ryzen's tables:** with `MIND_AMD_VBIOS` naming the Ryzen's VBIOS image, they also run the Ryzen's own
  tables.
  - The copy the owner session kept was under `/tmp` and went with the restart.
  - The image must never enter the repository, a commit, an issue or a message.
  - **Getting it back:** it is the image inside the ACPI VFCT table: offset 52 names its header, the header's bytes 24
    to 27 its length, and the image follows the header's 28 bytes, as `atom::vfct_image` reads it.
  - **Where to look for that table:** the Ryzen's `init` copies the ACPI tables to `log:acpi/` at every boot (`ACPI
    TABLES: 26 IN log:acpi/`). Fetching it from there is the owner session's (the Effector server or the cable).
- **The full suite:** `scripts/host_tests.sh` runs them all. On LEG9 its ssh tests need
  `MIND_SSHD=/tmp/claude-1000/sshd-pkg/root/usr/sbin/sshd`, which the owner session puts back after a restart. Heavy
  runs on LEG9 take `/tmp/claude-1000/heavy.lock` with `flock`.

**The hardware evidence:**
- **The survey's order:** boots 70 to 81 (0092, 0095).
- **The production order:** boots 84 to 89 and 91, the release of a71a47b83, 6.16 to 6.18 s from `display`'s start to
  the offer, every watch clean (0098's table).
- **The standing check:** boot 93 (above), 50 minutes, nothing found.
- **The logs:** the owner session's copies under `/tmp` were wiped with the restart, but for boot 93's, which the owner
  session kept under its handoff directory. The boots' own logs stay on the Ryzen's log volume (`BOOTNNNN.LOG`).

**What waits:**
- **For the maintainer:**
  - 0096's run at the monitor, with the questions on standby in its issue;
  - his decision on 0099's proposal;
  - 0097's cold path, which is his decision;
  - his look at the 3440x1440 picture (0095's criterion 4), now with `wm` in front.
- **For the owner session:**
  - `SHORT_WATCH`'s value for the bench, after boot 93;
  - the VBIOS image fetched again for the display suites' Ryzen cases;
  - closing 0098 once it has landed.

## Acceptance criteria

- [x] Host cases: nothing for an hour; an underflow after the offer caught within a second and undone as the watch does,
  the mark kept; the firmware's pipe and a request timeout alike; a transient that is over within the second still
  counted by its latched status; an underflow one frame after the watch ends caught within a second; the watch 300
  unless 60 is given with the check.
- [ ] On the Ryzen: the check running after the offer, its ten-minute line seen, nothing found, with the watch of 300;
  then the owner session's choice of the bench's watch. The first part was seen on boot 93: 50 minutes, five lines,
  nothing found. The choice is still to come.

## Related

[213-DRV-0098](213-DRV-0098-display-both-screens-at-start-in-seconds.md) (the watch's numbers, the gap at `init`'s end
of boot), [213-DRV-0096](213-DRV-0096-display-the-second-output-plugged-and-unplugged.md) (the loop it rides on),
[213-DRV-0092](213-DRV-0092-display-mind-cores-picture-on-the-second-output.md) (the watch, boot 74's underflow).
