# 800-KRN-0173 — With `dev`, `link` writes `data/firmware/`, so that a driver's proprietary file goes over the cable

**Type:** `init` · **Owner:** `KRN` · **Priority:** P1 · **Status:** open (landed in round 37, `main` c8fe8d03, both architectures gated; the run on a machine left, LNK's) · **Blocked by:** — · **Main task:** [800](800-mind-link.md) (the link track's) · **Roadmap:** track C (update and distribution) · **Constitution:** MC-2.3, MC-3.1, MC-10.2, MC-12.1, MC-12.3

**Asked by:** the link track (`LNK`) on 2026-10-27, for 800-LNK-0027 (`claude/LNK-link` 6696a1eb), in `KRN/2026-10-27T2156Z-LNK-a-scope-of-data-firmware-for-link.md` on `internal-dev-messages`.

## Problem

- **The maintainer (2026-10-27):** `effector485` was built so that a machine with slots is never written again; it is updated over the cable alone.
- **What the cable carries today:** a release with its own loader, a service replaced until the next boot, and files in `data/link/`.
- **What it does not: proprietary firmware in `data/firmware/`** (AGENTS.md, section 3).
  - `link` reaches `data/link/` alone (800-KRN-0128).
  - The shell's client for the link refuses `mv` and `mkdir`.
- **The case:** on 2026-10-27 the DEXP's stick was written again only because `rtw88_wifi`'s firmware was new (550-DRV-0042).

## What is granted, and why it widens nothing in kind

- **With `dev` in `link.conf`:** `init` gives `link` a second writable `vfs_server` client, confined to `data/firmware/` and badged `BADGE_USER` as its `data/link/` client is, in `SLOT_FILE` (7), which `link` held nothing in.
  - A new one at each start, as for `data/link/` (750-KRN-0120).
  - The directory is made when it is missing.
  - `[INIT] link: DEVELOPER MODE …` names it: `data/firmware/ TO WRITE`.
- **Without `dev`:** nothing changes; `link` reaches `data/link/` alone.
- **No new authority:**
  - **The user's badge** already writes anywhere in the boot disk's `data/` (vfs_server's zones), `data/firmware/` included.
  - **`dev` already gives more:** the cable's holder may replace any driver with an image nothing verified (`replace`, 800-KRN-0144), with the driver's own device, DMA and interrupt.
- **What checks a file put this way:**
  - each Wi-Fi driver reads it through its own read-only client;
  - `rtw88_wifi` and `rtw89_wifi` refuse a file whose size or SHA-256 differs from their pin;
  - **`bcm_wifi` does not pin its microcode:** it logs the file's SHA-256 and checks only its format. A file put over the cable reaches it as one copied at the write does. The request's text says all three pin, so this is told to `LNK`.
- **What stays as it is:** proprietary files never enter the repository or an image (AGENTS.md, section 3). The cable only puts the maintainer's fetched files onto the maintainer's own machine, as `scripts/proprietary.sh copy` puts them after the write. AGENTS.md's wording is the maintainer's to change; `LNK` proposes it.

## Plan

1. `init/src/main.rs`:
   - `FIRMWARE_DIR`;
   - in `spawn`, with `link_dev()`, the scope in `SLOT_FILE`, or `[INIT] link: NO FILES IN data/firmware (…)` when `vfs_server` refuses it;
   - the developer mode's line.
2. `tests/qemu_smoke.py`:
   - the `link` suite (with `dev`): slot 7 is a client of `vfs_server`, init's line names the scope, and `data/firmware/` exists;
   - the `linkupdate` suite (without `dev`): slot 7 stays empty, which its check already enforces.
3. `docs/profile/bootstrap.md`: the scope, and what checks a file put through it.

`link`'s names under `firmware/`, `proprietary.sh link` and the docs are 800-LNK-0027's.

## Acceptance criteria

- **Builds:** x86 and aarch64.
- **QEMU, x86:** the `link` and `linkupdate` checks pass with the criteria above.
- **On a machine** (`LNK`'s part, 800-LNK-0027): a firmware file put over `effector485` is read by its driver after a restart. The machine and the build are named (MC-12.1).

## Related

800-LNK-0027 (`issues/800-LNK-0027-firmware-over-the-link.md` on `claude/LNK-link`), [800-KRN-0139](800-KRN-0139-the-links-developer-mode.md), [800-KRN-0144](800-KRN-0144-a-service-replaced-from-data-link.md), [550-KRN-0172](550-KRN-0172-rtw88-wifi-boot-image-and-grant.md).
