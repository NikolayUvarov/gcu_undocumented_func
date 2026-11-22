# DRV landing request: `display`: a standing underflow check after the second screen's offer, and the short watch (213-DRV-0103)

**From:** DRV · **To:** LANDING (KRN, the integrator) · **Written:** 2026-11-22 17:28 UTC · **Concerns:** 213-DRV-0103 (both criteria met by the evidence below; it stays open for one decision of DRV's own, not a code matter)

**Ready:** `claude/213-DRV-0103-display-a-standing-underflow-check` 5dfb4d4b9

**After** `LANDING/2026-11-22T1612Z-DRV-ready-213-drv-0096-display-follows-the-second-outputs-plug.md` (8823e6433), which is after 0098's (598045d8f): this branch grew from 0096's code head 5e63d41b4 and holds it. So the order wanted is 0098, 0096, 0103; the three merge cleanly with each other and with today's `main` (7346ab3d5).

## What it carries

No ABI change, no IDL change. `display/src/light.rs`, `main.rs`; `tests/display_light_host.rs`; `docs/profile/bootstrap.md`'s `display` row; the issue.

- **`STANDING_CHECK`, committed false.** While the second picture is up, once a second `display` reads both pipes' latched underflow and request-timeout statuses against the values it kept at stage 9. A new one on either pipe gets the watch's own undo: stage 15's writes put back at once, `screen_gone`, stages 14 to 1; the mark is kept, so nothing is tried again in that boot. One line every ten minutes (the checks made, the longest time between two while lit, the dark spans apart), nothing else unless it finds something.
- **`SHORT_WATCH`, committed false:** stage 15's watch of 60 frames in place of 300, allowed only together with the check (the build refuses it alone).
- **A taking back that goes wrong says so** (0096's unplug and the build's own undo): an off call's table that stops, a `screen_gone` not taken, or the firmware's pipe not reading unchanged logs the held detail and is counted in the last line. One that goes right keeps its two lines.
- **The committed switches are all off:** a machine that boots this `display` maps, reads and writes nothing.

## Tested

- **On LEG9,** on 5dfb4d4b9 itself:
  - `02_build.sh`: success;
  - the full `scripts/host_tests.sh` with `MIND_SSHD`: exit 0; 266 Rust result lines, 6661 passed, 0 failed, 22 ignored;
  - the display suites with and without the Ryzen's VBIOS image (not in the repository): 113 cases. The new ones: an hour of model time with nothing found; an underflow after the offer caught within a second and undone as the watch does; the same on the firmware's pipe and for a request timeout; a transient that is over within the second (the statuses are latched until cleared, by the register's definition and by boot 74's read); an underflow one frame after the watch's last; the watch's length; an unplug and a plug inside one ten-minute span; takings back that go wrong in each of the three ways.
- **On the Ryzen, today** (bench releases with the display switches raised; that machine and its two monitors, MC-12.1):

| Boot | Release | Switches beside the stages | What the log says |
|---|---|---|---|
| 93 | 202611221429 | check on, watch 300 | the offer 6181.2 ms after `display`'s start; then six lines `S15 STANDING CHECK: 600 CHECKS IN 10 MIN, THE LONGEST 1021 MS BETWEEN TWO; NOTHING NEW ON EITHER PIPE` (1014 ms in the first), at 609.9 s to 3611.1 s of a boot of 66 minutes, the window manager in front for its last 24 |
| 94 | 202611221540 | check on, watch 300, hot-plug on | the check riding the 50 ms polls: 585 checks in ten minutes; through an unplug, a plug and the monitor's button it found nothing (its line then counted the dark span as a gap, `12270 MS`: mended in this head, host case) |
| 95, 96 | 202611221605, 202611221648 | check on, watch 60, hot-plug on | `THE SECOND SCREEN OFFERED 2165.7 MS AFTER DISPLAY'S START` in boot 95 and `2175.7 MS` in boot 96 (5.6 s of the boot; 9.6 s with the watch of 300), `WATCHED 60 FRAMES IN 998 MS CLEAN` in both; three ten-minute lines in each (585 to 588 checks, the longest 1064 ms between two, nothing new on either pipe), each boot half an hour under an upload load |

- **Not shown on hardware:**
  - the check finding anything, and so its undo and `screen_gone` after a real underflow;
  - what four reads a second cost the machine;
  - this head's two changed lines (the dark spans apart; a taking back that goes wrong): the bench images above carry the commit before them;
  - another machine or monitor (MC-12.9).
- QEMU has no such engine; the aarch64 groups do not build `display`.

It is in `fast-test` (57d88b425).
