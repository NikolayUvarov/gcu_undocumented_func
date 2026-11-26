# 213-DRV-0111 — `display` sleeps between its polls: a light-up no longer holds the CPU the mice share

**Type:** driver (`display`) · **Owner:** `DRV` · **Priority:** P2 · **Status:** in progress (2026-11-26: the change and its host cases are on the branch; the run on the Ryzen is to come) · **Blocked by:** — · **Main task:** [213](213-amd-ryzen-pc.md) · **Roadmap:** the drivers track · **Constitution:** MC-12.1, MC-12.2, MC-12.3

## Problem

On the Ryzen the pointer moves in jerks each time the second monitor is detected. The maintainer, 2026-11-26, after
taking the HDMI plug out and putting it back several times on boot 107: «Заметил, что мышь тормозит в момент
определения монитора - движется рывками.»

**What `display` does at a light-up.** Step 3 takes about 2 s (`STEP 3'S TIMES (MS): IN ALL 2028.0; … 9 509.2, … 15
1040.3`). For about 1.5 s of that it holds its CPU without a pause:
- **Stage 9** counts 30 frames of the firmware's timing generator (0.5 s), reading the frame count every 100 us. The
  service's clock waits anything under a millisecond awake (`spin_loop`).
- **Stage 15's watch** reads the frame count for 60 frames (1 s) in a loop with no wait at all.

**Whom that keeps waiting.** A task stays on the CPU it was started on. On boot 107 `ps` shows CPU 6 shared by
`usb_host` (PID 7), `bt_hci` (PID 23) and `display` (PID 39). Both mice pass through those two: the USB receiver's
reports through `usb_host`, the Bluetooth mouse's through `bt_hci`, whose transfers `usb_host` also carries. While
`display` holds the CPU they run only when its time slice ends.

**The kernel's lines for the minutes of four plug changes** (boot 107, minutes 33 and 34; an untouched minute has 2 to
13 waits behind another task):
- minute 33: `58 BEHIND ANOTHER TASK (MOST 64 MS, BEHIND PID 5)`, the longest late wait 81 ms;
- minute 34: `67 BEHIND ANOTHER TASK (MOST 10 MS, BEHIND PID 7)`.

The line names only the task behind the longest wait. PID 5 is the compositor, which held its own CPU for 64 ms once
in minute 33; that is not `display`'s and is told to `KRN`. What share of the jerks each has is not measured.

## Plan

1. Stage 9 waits awake only for its two edges, a frame at most each, and sleeps through the frames between. It counts
   from one edge to another 30 frames later or more, so a wake that comes late only makes the count longer. The edges are
   polled every 20 us in place of 100, as the awake part is now short: the count is more exact than before.
2. The watch sleeps a millisecond when no new frame has come. The underflow and timeout statuses are kept until they
   are cleared, so none is lost; a wake later than a frame shows in the line's "passed between two reads" and in its
   longest gap, as a task kept from running already did.
3. The plan's status waits (`Step::Wait`, polled every 100 us) stay awake: some wait for a live bit that a sparse poll
   could miss. On the Ryzen they are 82 to 94 ms of a light-up, in pieces of 42 ms at most.

## Acceptance criteria

- [x] Stage 9 is awake for two frames at most and asleep for the rest of its count; a wake 40 ms late gives a longer
  count with the same pixel clock and reference clock (`display_light_host.rs`,
  `stage_9_and_the_watch_sleep_between_their_polls_and_a_late_wake_spoils_neither`).
- [x] The watch sleeps between its looks and reads every frame when woken on time; a wake 40 ms late marks the frames
  slept through and names the gap; an underflow during such a sleep is found at the next look (the same case). With the
  polls awake again the case fails.
- [x] The other cases of `display_light_host.rs` pass unchanged, the one on boot 70's values among them (its sync
  values depend on the count to 0.01 %).
- [ ] On the Ryzen: the second output lights in its monitor's mode with the same lines as before, the plug taken out
  and put back while the mouse is moved, and the maintainer's look at the pointer (MC-12.1).
- [ ] The kernel's minute line for that minute, against minutes 33 and 34 above.

## Related

[213-DRV-0095](../issues-done/213-DRV-0095-display-the-second-outputs-mode-from-its-edid.done) (the light-up this
changes), [213-DRV-0103](../issues-done/213-DRV-0103-display-a-standing-underflow-check.done) (the standing check, which
already sleeps), 000-KRN-0276 and 000-KRN-0283 (late wakes on an idle CPU: what makes a millisecond's sleep longer).
