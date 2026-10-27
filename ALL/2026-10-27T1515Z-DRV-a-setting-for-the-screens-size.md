# A request: a setting for the screen's size, kept on the disk and applied by the bootloader

**From:** DRV (passing on the maintainer's word) · **To:** KRN (`PRT`, the bootloader and `BootInfo`) and APP (the shell's command, `wm`'s settings) · **Written:** 2026-10-27 15:15 UTC · **Concerns:** `bootloader/src/main.rs`, `common/abi.rs` (`BootInfo`), `shell`, `wm` · **Answer wanted:** yes: the task numbers, from each track that takes a part

The maintainer asks for this (2026-10-27): "a task for the track concerned: add settings for the screen's size".

## Why

On the DEXP desktop the firmware left 800×600 on one boot and the monitor's own mode on another (`KRN/2026-10-27T1535Z-DRV-the-bootloader-keeps-800x600-on-the-dexp.md`). That request asks the bootloader to choose the monitor's own mode by itself. This one asks for the owner to choose, and to keep the choice.

## What is asked, as DRV sees the parts

1. **`PRT`, the bootloader:**
   - reads the owner's choice from the boot volume, as it reads `data/pause.txt`: for example `data/screen.conf` with `1920x1080`, or `auto` for the monitor's own mode;
   - sets that GOP mode when the GOP lists it with a linear framebuffer, and otherwise falls back and says so;
   - hands the system the modes the GOP lists, so the shell can offer them (a `BootInfo` field: `KRN`'s ABI step).
2. **`APP`, the shell and `wm`:**
   - a command, for example `screen` to list the modes and the current one, and `screen 1920x1080 | auto` to write the choice for the next start;
   - an item in `wm`'s settings doing the same;
   - the change takes effect at the next start, and says so.
3. **Docs:** the setting in the shell's help and in `docs/`.

The names above are only examples. The format and the place are the owners' to choose.
