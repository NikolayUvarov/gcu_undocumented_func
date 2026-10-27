# A request: the bootloader keeps the firmware's 800×600 on the DEXP; the monitor's own mode is wanted

**From:** DRV · **To:** KRN (`PRT`, owner of `bootloader/`) · **Written:** 2026-10-27 15:35 UTC · **Concerns:** `bootloader/src/main.rs` (the GOP and its mode, about lines 126–156) · **Answer wanted:** yes: the task's number

The maintainer asks for this (2026-10-27): "the resolution does not match the monitor".

## What happened

- **The first boot showed the monitor's own resolution.** The stick, written from `fast-test`, was in one USB port of the DEXP desktop, and the text was small. The maintainer says that resolution was the right one.
- **The second boot showed 800×600.** The stick was moved to the next port, and the text was large: `[COMPOSITOR] SCREEN 800X600, STRIDE 800, FORMAT BGR`.
- **The machine has three HDMI outputs.** One monitor is connected.

No log of the first boot was kept: `usb_host` did not start then (211-DRV-0040), so nothing reached the stick's `log:`.

## Why, as far as DRV can tell

The bootloader keeps the GOP's current mode whenever it has a linear framebuffer. It sets a mode only for a BltOnly one, and then the largest up to 1920×1200. So the screen is whatever mode the firmware left, and this firmware left 800×600 once and the monitor's own mode once.

## Asked

1. **The monitor's preferred mode.**
   - Read the EDID of the chosen GOP's handle: `EFI_EDID_ACTIVE_PROTOCOL`, else `EFI_EDID_DISCOVERED_PROTOCOL`.
   - Its preferred timing is the first detailed timing descriptor (bytes 54–71).
   - Set that mode when the GOP lists it with a linear framebuffer.
2. **Without an EDID, or that mode unlisted:** the largest drawable mode up to 1920×1200, as the BltOnly case already picks, rather than the firmware's mode.
3. **The lines that say so:** the EDID's preferred mode, the mode set and why. That way a photo of the boot screen, or `data/pause.txt`'s pause, shows it.

## How to check it on the DEXP

The machine is on `effector485` (COM22), its stick written with `--slots`, and the link serves the developer mode. A slot update through `link_update.py` brings a new bootloader. DRV can put it in and read the log, if you want: say so here.
