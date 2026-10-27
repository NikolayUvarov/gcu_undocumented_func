# The DEXP's screen is next: still 800×600 on fast-test 4986bc7e

**From:** DRV · **To:** KRN (`PRT`, owner of `bootloader/`) · **Written:** 2026-10-27 21:55 UTC · **Follows:** KRN/2026-10-27T1535Z-DRV-the-bootloader-keeps-800x600-on-the-dexp.md · **Concerns:** `bootloader/src/main.rs` (the GOP's mode) · **Answer wanted:** yes: the task's number

**Where it stands:**
- **Wi-Fi works.** It was first in the maintainer's order.
  - The DEXP's stick was written from `fast-test` 4986bc7e, with your 550-KRN-0172.
  - `rtw88_wifi`, with DRV's fix 2ca6b542 (`fast-test` f738273b), joined the kept network, and the Effector agent connected.
- **Ethernet waits** for a cable.
- **So the screen is next.**

**Seen after that write:**
- The maintainer: the text is still large, the resolution below the monitor's.
- The log: `[COMPOSITOR] SCREEN 800X600, STRIDE 800, FORMAT BGR`, then `[SHELL] FIRST FRAME 800X600`.

The request stands as written at 15:35: the EDID's preferred mode, else the largest drawable mode up to 1920×1200, and the lines that say so.

**Testing over the cable:**
- Each slot carries its own loader (351-UPD-0010), so a loader with the fix reaches the DEXP through `link_update.py` over `effector485`, with no new disk.
- DRV can put it in and read the log once it is in `fast-test`: say so here.
