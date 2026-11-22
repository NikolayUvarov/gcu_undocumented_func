# DRV landing request: `display` follows the second output's plug (213-DRV-0096)

**From:** DRV · **To:** LANDING (KRN, the integrator) · **Written:** 2026-11-22 16:12 UTC · **Concerns:** 213-DRV-0096 (both criteria met by the evidence below; I close it after it lands)

**Ready:** `claude/213-DRV-0096-display-the-second-output-plugged-and-unplugged` 8823e6433

**After** `LANDING/2026-11-22T1416Z-DRV-ready-again-213-drv-0098-displays-production-order-one-text-commit-more.md` (598045d8f, waiting): this branch grew from 0098's code head a71a47b83 and holds it; 0098's last two commits are issue text it does not carry, so both requests are wanted, 0098's first. A trial merge with today's `main` (7346ab3d5) is clean.

## What it carries

No ABI change, no IDL change. `display/src/hpd.rs` (new), `light.rs`, `main.rs`; `tests/display_hpd_host.rs` (new) and `display_light_host.rs`; `docs/profile/bootstrap.md`'s `display` row; the issue.

- **Behind `HOTPLUG`, committed false** (it also needs stages allowed, and the committed build allows none: a machine that boots this `display` maps, reads and writes nothing).
- After its stages `display` reads HPD 1's sense every 50 ms. A change counts once it has held 500 ms, so a bounce does nothing.
- **An unplug** takes the stages back, 16 to 1, last first, `screen_gone` first; the once-a-boot mark is put back as it was before step 3.
- **A plug** runs steps 1 to 4 again on the same mapping: the reads, the EDID, the stages, a new `offer_screen`.
- After a failure nothing more is tried in that boot. The mark now means "stages in place": a failure's undo leaves it, an unplug's taking back clears it.

## Tested

- **On LEG9,** on 5e63d41b4 (the commits after it change only the issue):
  - `02_build.sh`: success;
  - the full `scripts/host_tests.sh` with `MIND_SSHD`: exit 0; 266 Rust result lines, 6652 passed, 0 failed, 22 ignored; the Python and shell tests OK;
  - the display suites with and without the Ryzen's VBIOS image (not in the repository): 104 cases. The new ones: the settle (one action at the first poll past 500 ms; bounces to 450 ms and a sense flickering for 5 s do nothing); an unplug after stage 16 gives `screen_gone` before anything else and leaves every register as before step 3; a plug then makes the first start's writes and offer exactly; with the Ryzen's own tables, warm and cold; end to end on the model with the poll.
- **On the Ryzen, today** (release 202611221540, boot 94, a bench tree with this code and `HOTPLUG` raised; that machine and its KQ34C144C on HDMI-A-1, MC-12.1), the maintainer's hands on the plug, the log read over the cable:

| Seconds | What he did | What the log says |
|---|---|---|
| 9.627 | (start) | `HPD 1 FOLLOWED EVERY 50 MS, A CHANGE COUNTED AFTER 500 MS: SENSE 1, THE SECOND OUTPUT Lit` |
| 1044.701 | HDMI-A-1's cable out | `STEP 3 TAKEN BACK: HPD 1 SENSES NO MONITOR; UNDOING STAGES 16 TO 1 (179 WRITES)`; `[COMPOSITOR] SECOND SCREEN GONE (PID 38)` 26 ms later; `TAKEN BACK IN 30.9 MS: THE SECOND PIPE IS OFF, ITS MARK CLEARED; THE FIRMWARE'S PIPE UNCHANGED` |
| 1049.2–1055.433 | the cable in | steps 1 to 4 again: the EDID read at once, 3440x1440 in HDMI, 16 stages done, 300 frames clean, `[COMPOSITOR] SECOND SCREEN 3440X1440 …` at 1055.370, `A MONITOR PLUGGED IN: STEPS 1 TO 4 RAN AGAIN, THE SECOND OUTPUT Lit` |
| 1105.113 | that monitor switched off at its button (HPD low for about 0.6 s) | the same taking back, in 35.6 ms |
| 1105.7–1111.935 | (HPD up again by itself) | the same run again, `Lit` at 1111.935 |

  - **The maintainer:** «при отключении hdma и выключении монитора в обоих случаях через некоторое время появилось изображение заставки mind-core-os, а затем и отобразился обычный экран» and «первый экран не мограл, сейчас на них одно и тоже исзображение».
  - `display.wit`'s side worked as it stands: `screen_gone`, then a second `offer_screen` in one boot, twice.
- **Not shown:**
  - a monitor that keeps HPD low while it is off or asleep (this one pulses it for about 0.6 s at its button and holds it up; no third event came in the minute after);
  - another monitor or mode offered at the plug (the same monitor came back each time);
  - a failure inside a taking back or inside a plug's run, on hardware;
  - a start not through Linux (table 12's long path, 213-DRV-0097);
  - another machine (MC-12.9).
- QEMU has no such engine; the aarch64 groups do not build `display`.

It is in `fast-test` since 13:38 UTC (9f3a608cd, its code head 5e63d41b4).
