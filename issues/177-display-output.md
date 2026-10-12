# 177 — Display output: every monitor, where each one stands, modes of our own, then the GPU for the picture

**Type:** main task · **Owner:** `KRN` (the bootloader's screens, `BootInfo`, the kernel's display capabilities, the compositor), with `DRV` (display drivers) and `APP` (`wm`, the settings) · **Priority:** P2 · **Status:** open (plan) · **Blocked by:** — · **Roadmap:** tracks A, G and H · **Constitution:** MC-3.3, MC-5.1, MC-12.1, MC-12.3

## Problem

The maintainer (2026-10-12) asked whether several monitors and their positions are supported, and asked for this task.

**What exists:**

- The screen is the one framebuffer the firmware's GOP leaves at boot. The firmware chooses its mode. The CPU draws it: the compositor keeps a shadow copy and copies whole frames ([000-KRN-0086](../issues-done/000-KRN-0086-the-compositor-copies-whole-frames.done)).
- There is no display driver, so there is no mode change, no second monitor, no layout, no hot plug and no vertical sync.
- Main task [174](174-full-use-of-pc-hardware.md) plans the GPUs for computation (steps 5–8: the IOMMU, large BARs, `idl/accel.wit`, then NPUs), not for the picture.
- **The MacBook Pro 2012** has an Intel HD 4000 and an NVIDIA GK107. Its external ports are reported to be wired to the NVIDIA GPU, which 174 leaves out (it predates GSP). So an external monitor on that machine needs a Kepler display driver; the hardware report should confirm which GPU drives which connector first.

## Plan

| Step | Track | What |
|---|---|---|
| 0 | `KRN`, `PRT` | **Know the outputs.** The hardware report lists every GOP the firmware offers (its modes, the one set, the device it belongs to) and each connected monitor's EDID (`EFI_EDID_DISCOVERED_PROTOCOL`), read by the bootloader before it leaves boot services. |
| 1 | `KRN` | **Every screen the firmware lit.** The bootloader passes every GOP framebuffer with its EDID in `BootInfo` (a new ABI version). The kernel hands each out as its own display capability, and the compositor holds them all. Where the firmware lights one output, as on most machines, nothing changes. |
| 2 | `KRN`, `APP` | **Layout.** `data/displays.txt` names each monitor by its EDID identity, with its place (left of, right of, above or below another), which is primary, its scale (1× or 2×) and its rotation. The compositor maps one desktop over the outputs: the pointer crosses between them, and each window lands on one. `wm` knows each output's rectangle (a new version of `window.wit` / `display.wit`). A `displays` tool and `wm`'s Settings change the file. |
| 3a | `DRV` | **A first driver of our own: `virtio-gpu`** (2D: resources, scanouts, transfers and flushes, the display-change event). QEMU runs it with `max_outputs=2` on x86 and aarch64, so steps 1–2 and hot plug are tested in CI before real hardware. |
| 3b | `DRV` | **Modes on Intel graphics**, from Intel's public PRMs: the display engine's pipes, planes and ports. A mode is set and changed, an output the firmware left dark is lit, hot plug is seen, a monitor is put to sleep. The MacBook Pro's HD 4000 (Ivy Bridge) and a current Intel PC are the evidence. |
| 3c | `DRV` | **AMD (DCN)** from AMD's documentation, with the MIT-licensed Linux `amdgpu` as reference. **NVIDIA** only from the GSP generation on, as in 174. The Kepler display engine (the MacBook Pro's external ports) is a separate decision. |
| 4 | `KRN`, `DRV` | **Page flips and vertical sync:** the compositor shows a finished frame at the next vertical blank, with no tearing (the display fences of roadmap track G). |
| 5 | `KRN`, `DRV` | **The picture on the GPU:** once 174's accelerator service exists, the compositor blends and scales on the GPU, under the same per-program budgets (MC-5.1). |

Each track numbers its own part when it starts it. Only the compositor holds scanout memory; programs draw into their own surfaces as now (MC-3.3). Each step adds the `docs/profile` lines it changes and is tested on the machines it names (MC-12.1).

## Acceptance criteria

- **The hardware report** lists every output and monitor, with its EDID, on the MacBook Pro and on a PC with two monitors.
- **QEMU** (`virtio-gpu`, two outputs): the desktop spans both as `data/displays.txt` places them; a third output added at run time is lit and placed.
- **A PC whose firmware lights two outputs** shows one desktop over both, without our own driver.
- **An Intel PC with the driver of step 3b:** a monitor plugged in at run time lights up, and its mode can be changed.
- **Frames shown at vertical blank** do not tear (a test pattern, judged by eye and by the driver's counters).

## Related

[174](174-full-use-of-pc-hardware.md) (the GPUs for computation), [000-KRN-0086](../issues-done/000-KRN-0086-the-compositor-copies-whole-frames.done), [000-KRN-0081](000-KRN-0081-the-framebuffer-written-combined.md), [211-KRN-0018](../issues-done/211-KRN-0018-the-compositors-quota-fits-the-screen.done), [ROADMAP.md](../ROADMAP.md) track G.
