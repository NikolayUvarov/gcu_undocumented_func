# 009 — Honour `PixelFormat` and select a GOP mode

**Type:** bug/robustness · **Priority:** medium · **Status:** open
**Affects:** `bootloader/src/main.rs:35-45`, `kernel/`, `app/`

## Problem

The bootloader takes `current_mode_info()` and passes only `width/height/stride`. The kernel and app write bytes in `B, G, R, X` order (`kernel/src/main.rs:51-54`), i.e. they assume `PixelFormat::Bgr` and 32 bpp. GOP may return:
- `PixelFormat::Rgb` — colours get swapped (not critical, but wrong);
- `PixelFormat::Bitmask` — arbitrary masks;
- `PixelFormat::BltOnly` — there is **no** linear framebuffer; writing to `fb_ptr` is UB/a hang. On real hardware with some firmware this is a real case.

`stride` is passed in pixels — the kernel correctly multiplies by 4, but the contract is not documented anywhere.

## Plan

1. Add `pixel_format: u32` (0=Rgb, 1=Bgr, 2=Bitmask, 3=BltOnly) and the masks for Bitmask to `BootInfo`; on `BltOnly` — iterate over `gop.modes()` and pick a mode with a linear buffer, otherwise report it and halt.
2. Mode selection: prefer the maximum resolution with `Rgb|Bgr`, or a target one (1024×768) for predictable performance of per-pixel rendering.
3. In kernel/app — a format-aware `put_pixel` (see [007](../issues-done/007-kernel-font-and-primitives.done)).
4. Document: `stride` is in pixels, 4 bytes per pixel.

## Acceptance criteria

- Running QEMU with `-vga std` and `-vga virtio` (and `-device ramfb`) gives correct colours.
- On `BltOnly` there is a message on screen/serial instead of a hang.
