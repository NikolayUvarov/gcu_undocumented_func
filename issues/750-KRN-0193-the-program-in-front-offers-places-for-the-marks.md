# 750-KRN-0193 — `display.wit` 1.3 `offer-marks`: the program in front offers places for the marks, the corners otherwise

**Type:** `compositor`, `init`, IDL (`display.wit` 1.3) · **Owner:** `KRN`, made by `EFF` by agreement (section 2 of AGENTS.md) on EFF's branch · **Priority:** P2 · **Status:** open · **Blocked by:** [750-KRN-0192](750-KRN-0192-the-compositor-learns-whose-screen-is-in-front.md) · **Main task:** [750](750-effector-agent.md) · **Roadmap:** track G · **Constitution:** MC-3.11, MC-10.2, MC-12.4

**Asked by:** EFF on 2026-10-24, in `ALL/2026-10-24T1654Z-EFF-a-request-the-program-in-front-offers-a-place-for-the-marks.md` on `internal-dev-messages`. That request passes on the maintainer's choice of that day and replaces a34c20e9, 19a5ae12 and the place in 2c03c6d0. EFF offered on 2026-10-30 to make the compositor's part as a KRN task, and KRN agreed.

## Problem

- **The fault.** The compositor draws the remote mark, the capture dot and the camera mark at fixed pixels, so in some of `wm`'s modes they lie over its battery chip.
- **The maintainer's choice:**
  - the program in front leaves room for the marks, and the compositor draws them there;
  - with no room offered, it draws them in the corners, over the picture.

## Plan

- **`display.wit` 1.3:**

  ```wit
  /// A place in the caller's own screen, in its pixels.
  record place { x: u32, y: u32, width: u32, height: u32 }
  /// Where the caller's screen has room for the marks (1.3); none: the corners.
  offer-marks: func(remote: option<place>, capture: option<place>) -> result<_, error>;
  ```

  - The error enum gains `remote-place` and `capture-place`: the refusal names the place that does not suit, the remote one first.
  - An offer is taken or refused whole. Nothing of a refused offer is kept.
  - The new version and its transition follow MC-12.4, with the generated files made by `scripts/mind_idl.py`.
- **Which offer counts:**
  - the compositor keeps one offer per caller's PID (the IPC sender), the last one, for up to 8 PIDs, the oldest dropped first;
  - it uses the offer of the PID that `COMPOSITOR_PULL` names as in front (750-KRN-0192);
  - a new offer replaces the caller's old one, `none` for both removes it, and a PID is never reused, so an offer ends with its task.
- **What the compositor checks:**
  - a place lies within the screen's top band: the top 64 px, or a tenth of the height where that is larger;
  - it is at least as large as its marks at the compositor's scale: the remote mark alone; the capture dot and the camera mark side by side.
- **What the compositor never does:**
  - never takes an offer as a reason not to draw: a lit mark is always drawn, in its place or in its corner;
  - never draws a mark without a 1-px contrasting edge, since the place's background is the program's.
- **With no offer:** the remote mark in the top left corner; the capture dot and the camera mark in the top right, where they are now.
- **The remote mark's shape** (from a34c20e9):
  - a filled disc: the Effector agent alone;
  - a ring: the link alone;
  - a ring with a dot in the middle: both.

  The ring takes the link's colour and the disc or dot the agent's: grey, yellow or red, as in 1.2. `remote-active`'s blinking (2c03c6d0) stands.
- **The log:**
  - `[COMPOSITOR] MARKS IN THE PLACES PID <n> OFFERED` or `[COMPOSITOR] MARKS AT THE CORNERS` when that changes;
  - `[COMPOSITOR] MARKS OFFER REFUSED: PID <n>, <place>`;
  - the lines `REMOTE MARK …` that EFF's `effector` suite reads stay as they are.
- **`init`:** gives `wm` a client of the compositor in a slot of its own, as it does for `record`. The shell holds one in `SLOT_DISPLAY` already.

## Acceptance criteria

- **Host tests:** the generated `display.wit` 1.3 code, a fuzz entry for `offer-marks`, and the place checks as a function with its own tests.
- **QEMU, x86 and aarch64 (`display` checks):**
  - with no offer, the marks at the corners;
  - an offer from the program in front moves them into its places;
  - another program in front with no offer puts them back at the corners;
  - a place outside the top band, or too small, is refused naming that place, and the previous offer stays;
  - a lit mark is never missing from the screen.
- **`wm`'s offer end to end:** APP's 750-APP-0110, once this is in `main`.

## Related

[750-KRN-0192](750-KRN-0192-the-compositor-learns-whose-screen-is-in-front.md), [800-KRN-0156](800-KRN-0156-the-remote-marks-in-the-compositor.md), [750-APP-0110](750-APP-0110-wm-offers-its-top-bar-for-the-marks.md), [750-EFF-0015](750-EFF-0015-the-agents-mark-on-the-screen.md), issue 165 (the capture dot), issue 158 (the camera mark).
