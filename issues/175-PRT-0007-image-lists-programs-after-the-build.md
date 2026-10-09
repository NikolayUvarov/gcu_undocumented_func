# 175-PRT-0007 — The USB image lists the programs after the build, not before it

**Type:** porting (image scripts) · **Owner:** `PRT` · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Main task:** [175](175-audit-2026-10-09.md) · **Constitution:** MC-12.1, MC-12.9

## Problem

Audit finding A01 ([audit](../issues-audit/2026-10-09-repository-audit.md), [assessment](../issues-audit/2026-10-09-repository-assessment.md)):
- `scripts/make_usb_image.py` computes `APPLICATIONS` at import (line 29), before `main()` runs `02_build.sh` (lines 310–313). The x86 branch of `files()` reuses that list.
- So a clean clone's `./04_make_usb_image.sh --force` packs only the boot services. On 2026-10-08 that is what happened with an image for the MacBook Pro.

## Plan

- `files()` lists the programs from the chosen source after the build has run.
- A test packages a clean tree in one call, and again after a program is added. It checks the image against the build's own outputs, not against the packager's list.
- `issues-audit/repro/build_repro.py` becomes that regression test.

## Acceptance criteria

- One `make_usb_image.py --force` call on a clean tree gives an image with every program the build made.
- The test fails on the old code and passes on the new.

## Related

[011](../issues-done/011-reproducible-toolchain.done), [211](211-intel-pc-from-a-sata-ssd.md).
