# 000-APP-0100 — `wm`'s combined background: the image under the moving pattern

**Type:** tools (`wm`) · **Owner:** tools track (`APP`) · **Priority:** P2 (the maintainer's request, 2026-10-21) · **Status:** open · **Blocked by:** — · **Main task:** — · **Roadmap:** track G · **Constitution:** MC-11.5

**Recorded by:** the tools track, 2026-10-21, from the maintainer's request: "Can we have combined background - moving animations and image at same time, so when animation change brightness so in combained mode it change brightness of the background in same way?"

## Problem

- **The background is one or the other.** `data/wm.conf` chooses `abstract`, a moving pattern, or `image <file>`, a still picture ([000-APP-0047](../issues-done/000-APP-0047-wm-desktop-background.done), [000-APP-0050](../issues-done/000-APP-0050-wm-background-pictures-and-pattern-settings.done)).
- **What the maintainer wants:** the picture shown, with the pattern's light moving over it: brighter where the pattern is light, darker where it is dark.

## Plan

- **A fourth picture,** `background = combined <file>`.
  - Each block of the frame takes the pattern's level there, as `abstract` would.
  - That level gives a gain, and the picture's pixels are multiplied by it, every channel alike, so the colours keep their shares.
  - `contrast` sets how far the gain goes: from 100 − contrast to 100 + contrast percent (80 to 120 % at the default 20).
  - The pattern's kind, speed and complexity are the same keys as for `abstract`.
- **The picture is read once.** Switching between `image` and `combined` with the same file does not read it again. A picture that cannot be read gives the pattern alone, with a notice, as for `image`.
- **It moves** as `abstract` does: drawn every 500 / speed ms, its frame time logged (000-APP-0091).
- **Settings:** the Picture row cycles none → abstract → image → combined. The file row and the picture browser keep the kind chosen. The contrast row says what it does under `combined`.

## Acceptance criteria

- **Host tests** (`tests/wm_host.rs`):
  - the gains at contrast 0, 20 and 100;
  - a picture of one colour under the pattern: lighter wherever the pattern is lighter, its colour's shares kept;
  - contrast 0 gives the picture as it is;
  - the frame moves with the phase;
  - the picture kept between `image` and `combined`, a missing one giving the pattern;
  - the configuration read back as written;
  - the Settings cycle, the file row and the browser keeping `combined`.
- **The `wm` suite in QEMU:**
  - `background = combined data/sky.bmp` under the aurora at speed 20: each half of the picture keeps its colour's order, lit and dimmed, and the light moves between two screenshots;
  - its frame time is logged;
  - Settings' Picture row passes through `combined`.
- **Docs:** `docs/tools/README.md` and `README_RU.md`.

## Progress (2026-10-21)

- **Done:** `background = combined <file>`, Settings' cycle, docs. The host tests pass.
- **Locally,** "x86: keys, shell, tools" passed on 543ea7ea:
  - the `wm` suite's combined desktop: 6600 of 6600 points kept their half's colour order, the light moved, the first frame in 134 ms;
  - Settings passed through `combined`.
- **Found in the first run** (1f143a06): `settings_react` stepped the Picture row from image straight to none. It now steps through combined.
- **Left:** a real screen. These were QEMU's 1280 × 800 (MC-12.1).

## Related

[000-APP-0047](../issues-done/000-APP-0047-wm-desktop-background.done), [000-APP-0050](../issues-done/000-APP-0050-wm-background-pictures-and-pattern-settings.done), [000-APP-0091](000-APP-0091-wms-background-by-rows.md) (the frame time), [000-APP-0048](000-APP-0048-wm-settings.md) (Settings).
