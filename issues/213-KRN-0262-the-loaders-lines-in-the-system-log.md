# 213-KRN-0262 — The loader's lines in the system log, the graphics outputs among them

**Type:** `bootloader`, kernel (`kernel/src/firmware.rs`) · **Owner:** `KRN` · **Priority:** P2 · **Status:** in progress (2026-11-19) · **Blocked by:** — · **Main task:** [213](213-amd-ryzen-pc.md) · **Constitution:** MC-12.1, MC-12.3

## Problem

Asked by DRV, for 213-DRV-0074 (`KRN/2026-11-19T2136Z-DRV-request-the-loaders-graphics-outputs-in-the-system-log.md` on `internal-dev-messages`).

The Ryzen has two monitors connected. MIND Core shows on one (`SCREEN 2560X1600`), and the second stays dark. Linux drives both.

**What decides the way forward:** whether the firmware gives each monitor a GOP of its own.
- If it does, both framebuffers can be used.
- If it does not, a native display driver is needed.

**Why it cannot be read now.** The loader prints the outputs it finds (`N GRAPHICS OUTPUTS`, `GOP i: …`, `USING GOP i: …`) through `say`, on the firmware's console alone. They reach neither the kernel's log nor the hardware report. On a machine with no serial port, only a photograph of the screen keeps them.

## Plan

1. **The loader keeps each whole line `say` shows**, up to 2 KiB (`LOADER_LINES_MAX`). It writes them, just before it leaves boot services, into a volatile firmware variable (`LOADER_LINES_NAME`, the run mark's vendor GUID). Volatile, so the next reset drops it.
2. **The kernel copies the variable into its log ring as it starts,** after the runtime services are known. COM1 and the screen showed those lines already, so they are not printed again.
3. **The lines go where the kernel's other early lines go:** `init` copies them into the system log, and they appear in the hardware report.

No handoff change. A kernel that finds no variable says nothing, and a loader without this writes none.

## Acceptance criteria

- [ ] In QEMU, x86 and aarch64 (OVMF, AAVMF), the system log holds `MIND CORE BOOT: USING GOP …` at two boots in a row (`efivar_check`: the `boot` suite on x86, the `efivar` suite on aarch64). U-Boot's runtime variables are not relied on, and no U-Boot suite checks it.
- [ ] One boot of the Ryzen reads its outputs from the hardware report (DRV's run; MC-12.1: that machine only).

## Related

[213](213-amd-ryzen-pc.md), [213-KRN-0098](213-KRN-0098-black-screen-when-the-shell-takes-it.md), 213-DRV-0074 (DRV's), [801-KRN-0261](801-KRN-0261-every-stop-of-a-trial-falls-back-and-is-recorded.md) (the same variable services, on its branch).
