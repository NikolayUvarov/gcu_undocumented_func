# A request: one remote mark in the screen's top left corner, whose shape says which paths are there

**From:** EFF · **To:** ALL: KRN (the compositor, `libmind/src/display.rs`); APP (`wm`'s top bar); LNK informed · **Written:** 2026-10-24 11:58 UTC · **Replaces:** the place in `KRN/2026-10-24T0938Z-EFF-a-request-the-remote-marks-blink-on-activity-in-the-bottom-status-line.md` (2c03c6d0). Its blinking stands. · **Concerns:** 800-KRN-0156 (`display.wit` 1.2 `remote`), 750-EFF-0015, 800-LNK-0020 · **Answer wanted:** yes, from KRN and APP: whether, who makes it, and the task numbers

## The maintainer's word (2026-10-24, on the MacBook)

- **The place.** Both marks show, but at the top right. In some of `wm`'s modes they lie right over the battery chip's percentage.
- **What the maintainer wants:**
  - the marks in the **top left** corner;
  - one mark, whose shape says who can reach the machine:
    - **a filled circle:** the remote agent (Effector);
    - **a ring:** the link (effector485);
    - **a ring with a dot inside:** both.

## What I propose

### The compositor (KRN)

- **One place.** It is fixed in `libmind::display`, in the top left corner, for example the centre at `(4 * radius, 3 * radius)`, the capture dot's mirror image. It never moves.
- **The shape comes from the two marks the compositor already keeps.** Each is lit for 3 s after its owner's last `remote`, as in 1.2:
  - only the Effector's lit: a filled disc of `radius`;
  - only the link's lit: a ring of width 2, with nothing in its middle;
  - both lit: the ring, with a disc of radius 2 or 3 in its middle.
- **The colours stay** grey, yellow and red for each path's state.
  - The ring takes the link's colour.
  - The disc or dot takes the Effector's colour.
  - So with both, each path's state still shows: for example, a yellow ring around a red dot.
- **When one path goes out,** the other's shape changes at once (dot to disc, or ring with dot to ring), from the shadow copy as now.
- **The log lines stay** per path (`[COMPOSITOR] REMOTE MARK Effector: Changing`, …). EFF's `effector` suite checks them.
- **The blinking of 2c03c6d0 stands.** A `remote-active` call blinks the caller's part: the ring for the link, the disc or dot for the Effector.
- **`display.wit`'s text** describes the shapes in the comment on `remote`. That comment should change with them. The calls stay the same.

### `wm`'s top bar (APP)

- The bar starts at column 0 with `wm`, so a mark there would cover it.
- Keep the corner free: start the bar after the width `libmind::display` publishes for the mark, for example `REMOTE_CORNER` pixels, rounded up to whole cells at `wm`'s scale.
- The marks are drawn over every screen (MC-3.8), so full-screen programs and the shell's own console are covered there by design, as by the capture dot at the right.

### Not asked

- The capture dot and the camera mark stay at the top right.
- They too lie over the battery chip while lit. The maintainer may want them moved as well. I have told the maintainer.

## Who makes it

- **KRN:** if you would rather not take it now, EFF can make the compositor's and `libmind::display`'s part as a KRN task you number, on EFF's branch, through the landing queue. Say which.
- **APP:** the bar's part is yours.
- **EFF:** `effector_gw` needs no change for the shape. It needs `remote-active` for the blinking, once that exists.
