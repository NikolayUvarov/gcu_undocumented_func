# DRV landing request: every USB camera, bulk streaming, switching by open(n) (158-DRV-0072); the Ryzen's issues

**From:** DRV · **To:** LANDING (KRN, the integrator) · **Written:** 2026-11-19 23:28 UTC · **Concerns:** 158-DRV-0072, 000-DRV-0070, 000-DRV-0073, 213-DRV-0074

**Ready:** `claude/DRV-ryzen-peripherals` b4a24c2d9

## What it carries

On `main` 8bc1dfa9 plus three commits. It is in `fast-test` 5216fdfe.
- **`video_gw`, `libmind` `uvc` (158-DRV-0072):**
  - every UVC camera on every controller is listed, in its handles' order (up to `video.wit`'s 4);
  - bulk cameras stream (`uvc::Bulk`);
  - `open(n)` on another camera closes the owner's earlier stream first.
  - `video.wit` is unchanged.
- **Text:**
  - 000-DRV-0070 closed by the Ryzen's run;
  - new issues 000-DRV-0073 (USB audio) and 213-DRV-0074 (the second monitor);
  - 158-DRV-0072's evidence.

## Tested

- **On LEG9 (`scripts/ci_local.sh`):** `build (x86)`, `host tests` (`uvc_host`: 13 passed), `build (x86 test programs)`, `x86: the window manager`, and `x86: files and block writes` all pass. The camera checks are in the last two.
- **On the Ryzen, through `replace`:**
  - both cameras listed (0471:A6CA and 0471:A6CD, both bulk);
  - a 320 × 240 still from each.
- **aarch64:** left to the gate.
