# 211-KRN-0089 — The pauses for a photo of the boot screen only when `data/pause.txt` asks

**Type:** bootloader, kernel ABI (`BootInfo`), `init` · **Owner:** kernel session · **Priority:** P1 · **Status:** in progress (made, tested in QEMU; the MacBook Pro's boot left) · **Blocked by:** — · **Main task:** [211](211-intel-pc-from-a-sata-ssd.md) · **Constitution:** MC-12.4

## Problem

The maintainer (2026-10-12): "to speed up the boot, make a pause flag. If it is absent or zero, do not use the 5-second delays that were needed for photos of the screen."

Since [211-PRT-0004](211-PRT-0004-first-run-on-an-intel-pc.md), every boot on a machine without a hypervisor waited three times for a photo:

- the bootloader, 5 s after the firmware's lines;
- the bootloader again, 5 s after its own lines;
- `init`, 5 s before the compositor took the screen.

That is 15 s of every boot, now that no photo is needed.

## Plan

- **The flag.** The bootloader reads `data/pause.txt` on its own volume: the first number in it is the seconds of each pause, at most 60.
  - No file, or 0: no pause.
  - The file can be written from Windows on the stick, or in MIND with `write data/pause.txt 5` (the shell may write `data/`).
  - It is not in the signed manifest: it can only make the boot wait.
- **`init` learns it from the bootloader.** `init` cannot read the disk before the compositor starts.
  - The bootloader passes the value in `BootInfo.photo_pause_s`, a new field.
  - `ABI_VERSION` becomes 7 (MC-12.4).
  - The kernel hands every task's copy on as it is.
- **The machine's kind no longer matters.** The pauses followed CPUID's hypervisor bit; now only the file decides, so QEMU can test them.
- **A boot that fails** stops with its error on the screen, as before.

## Acceptance criteria

- **QEMU (x86, the `vfs` suite).** After `reboot` without the file, no pause is logged. With `data/pause.txt` holding 1, the bootloader logs `A PAUSE OF 1 S` twice and `init` logs its pause.
- **The `boot` suite** passes; the aarch64 image builds.
- **The MacBook Pro** boots without the three pauses.

## Progress

**2026-10-12, QEMU x86.**

- The `vfs` and `boot` suites pass.
- `tests/runtime.rs` checks ABI 7.
- The aarch64 image builds.

## Related

[211-PRT-0004](211-PRT-0004-first-run-on-an-intel-pc.md), [docs/write-disk.md](../docs/write-disk.md) §8, [docs/api/README.md](../docs/api/README.md) (ABI 7).
