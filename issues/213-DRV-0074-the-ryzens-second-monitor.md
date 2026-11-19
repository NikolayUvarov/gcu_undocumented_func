# 213-DRV-0074 — The Ryzen's second monitor stays dark

**Type:** display (`bootloader`, `compositor`; a display driver later) · **Owner:** `DRV`, with `KRN` for the loader and compositor · **Priority:** P3 · **Status:** open (2026-11-20) · **Blocked by:** — · **Main task:** [213](213-amd-ryzen-pc.md) · **Constitution:** MC-6.1, MC-12.1, MC-12.3

## Problem

**On the Ryzen, 2026-11-20** (`fast-test` 92c2c6e9), two monitors are connected. The shell shows on one (`SCREEN 2560X1600`), and the second stays empty. Linux's `amdgpu` drives both.

**Why:** MIND Core has no AMD display driver (Radeon iGPU 1002:1638 at 05:00.0). It draws into the one framebuffer the firmware set up. The loader picks one UEFI Graphics Output, as Linux's EFI stub does (`bootloader/src/main.rs`, `select_display`), and nothing lights another output after the firmware is gone. The loader lists the outputs it found (`N GRAPHICS OUTPUTS`, `GOP i: …`) on the boot screen only, not in the log, so this run's list is not known.

## Plan

1. **What the firmware offers:**
   - the loader's output lines written to the system log too (asked of `KRN`);
   - from Linux, the connectors and which monitor is on each (`/sys/class/drm/card*-*/status`, EDID).
2. **If the firmware gives each monitor a GOP with its own framebuffer:** the loader passes both, and the compositor shows the same screen on both, or extends it. This is `KRN`'s, and an interface change there gets a new `BootInfo` version.
3. **If not:** the second monitor needs a native display driver for the Radeon's display engine (DCN). That is a large task of its own and is not promised here.

## Acceptance criteria

- [ ] The run's graphics outputs recorded from the log, with the connectors as Linux names them.
- [ ] A decision recorded: two firmware framebuffers, or a display driver as a separate task.

## Progress (2026-11-20)

- **Linux on the Ryzen** sees two connected outputs, `card1-DP-2` and `card1-HDMI-A-1`. `DP-1` and `HDMI-A-2` are not connected. The firmware's framebuffer is one `simple-framebuffer` until `amdgpu` takes over.
- **The loader's GOP lines** reach the system log since KRN's 213-KRN-0262 (`fast-test` 87d9aadac9 on). That branch is ABI 35, and the Ryzen's stick is ABI 34, so they come with the next disk write.

## Related

[213-KRN-0098](213-KRN-0098-black-screen-when-the-shell-takes-it.md) (the shell's screen on this machine), 211-PRT-0011 (the monitor's preferred mode from EDID).
