# 000-APP-0111 — A setting for the screen's size: `screen` in the shell and a page in `wm`'s Settings

**Type:** tools (`shell`, `wm`) · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open · **Blocked by:** the bootloader reading the owner's choice and setting that GOP mode (`PRT`), and the modes the GOP lists handed to the system (`KRN`, a `BootInfo` field: an ABI step); neither numbered yet · **Main task:** — · **Roadmap:** the development of MIND Core on real hardware · **Constitution:** MC-12.3, MC-12.4

**Recorded by:** the tools track, 2026-10-28, from DRV's request `ALL/2026-10-27T1515Z-DRV-a-setting-for-the-screens-size.md` on `internal-dev-messages`, which passes on the maintainer's word of 2026-10-27: "a task for the track concerned: add settings for the screen's size".

## Problem

- **What happened.** On the DEXP desktop the firmware left 800×600 on one boot and the monitor's own mode on another. DRV's other request asks the bootloader to choose the monitor's own mode by itself.
- **What is asked here.** The owner chooses the screen's size, and the choice is kept across starts.
- **Whose parts.** The bootloader sets the GOP mode before the kernel starts (`PRT`), and only it can see the modes the GOP lists; the system learns them through `BootInfo` (`KRN`). This task is the command and the settings page.

## Plan

**The file and its format are the bootloader's** (`PRT` chooses them). DRV's example, which this task follows until `PRT` says otherwise: `data/screen.conf` holding `1920x1080`, or `auto` for the monitor's own mode.

- **`screen`** in the shell:
  - alone: the modes the GOP listed (from the new `BootInfo` field), the one in use (the framebuffer's width and height) and the choice in the file, with `auto` when there is none;
  - `screen <width>x<height>`: a listed mode written to the file, after the user agrees; one the GOP did not list is refused, naming the listed ones;
  - `screen auto`: the file says `auto`;
  - each change says `TAKES EFFECT AT THE NEXT START`, and the system log names it.
- **`wm`'s Settings**, a page **Screen**: the listed modes, the one in use marked, the choice taken with Enter or a click and written the same way.
- **Who may.** `screen` alone reads, and a client the shell lends gets it at once; a change is asked in the shell's window (`clients::taken_by`), and a script needs `requires: files`.
- **The bootloader's fallback** (a mode the GOP no longer lists, a monitor changed) is the bootloader's to say; `screen` shows what it said if `BootInfo` carries it.

## Acceptance criteria

- **Host tests:** the parsing of `screen`'s words and of the file's line; a mode not listed refused.
- **A QEMU check** (x86, OVMF's GOP lists several modes): `screen` lists them; `screen 1024x768` writes the file; after a reboot the framebuffer is 1024×768 and `screen` says so; `screen auto` brings the default back at the next start.
- **On the DEXP desktop:** the chosen mode kept across starts, stated with the date (MC-12.1).
- **Docs:** the shell's `help`, `docs/tools/README.md` and `README_RU.md`.

## Related

DRV's request on `internal-dev-messages` (above) and its companion to `KRN` about the bootloader keeping 800×600 on the DEXP; [177-APP-0094](177-APP-0094-wm-over-several-monitors.md) (`wm` over several monitors).
