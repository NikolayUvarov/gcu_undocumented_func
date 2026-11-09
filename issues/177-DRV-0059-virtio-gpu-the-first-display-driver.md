# 177-DRV-0059 — `virtio-gpu`, the first display driver of our own, with two outputs in QEMU (177's step 3a)

**Type:** driver · **Owner:** `DRV` · **Priority:** P2 · **Status:** open · **Blocked by:** `KRN`: [177](177-display-output.md)'s steps 0–1 (every output and its EDID in `BootInfo`, several display capabilities held by the compositor) · **Main task:** [177](177-display-output.md) · **Constitution:** MC-6.1, MC-12.1, MC-12.4

Numbered from the kernel track's request of 2026-10-12 in `requests-DRV.md` ("Display output over several monitors", the maintainer's request).

## Problem

**The maintainer asked** for several monitors, their places, and the GPU drawing the picture. 177 plans it in steps, and the drivers track's part is steps 3a–3c, 4 and 5:
- **3a** is this task;
- **3b**, Intel's display engine, is [177-DRV-0051](177-DRV-0051-the-dexps-intel-display-engine.md) for the DEXP;
- **3c** (AMD, and NVIDIA from the GSP generation), **4** and **5** get their numbers when they start, as 177's plan lists them.

**Why 3a first:** `virtio-gpu` is what QEMU runs with `max_outputs=2`. So steps 1–2 and hot plug can be tested in CI on x86 and aarch64 before any real machine.

## Plan

1. **`virtio_gpu`, a ring-3 driver, 2D:**
   - its resources, scanouts, transfers and flushes;
   - the display-change event;
   - a mode set on each output.
2. **It serves the display interface** 177's steps 1–2 set, in the version they set.
3. **The `display` suites in QEMU,** x86 and aarch64, with `-device virtio-gpu-pci,max_outputs=2`:
   - two outputs lit;
   - a second output plugged in at run time;
   - a mode changed.

## Acceptance criteria

The suites of plan step 3 pass on x86 and aarch64. No real machine is needed for this step.

## Related

[177](177-display-output.md), [177-DRV-0051](177-DRV-0051-the-dexps-intel-display-engine.md), [177-APP-0094](177-APP-0094-wm-over-several-monitors.md), [177-APP-0127](177-APP-0127-a-display-mode-chosen-by-hand.md).
