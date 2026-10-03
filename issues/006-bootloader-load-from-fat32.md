# 006 — Bootloader: read the kernel and the application from FAT32 instead of `include_bytes!`

**Type:** feature · **Priority:** medium · **Status:** open
**Affects:** `bootloader/src/main.rs`, `02_build.sh`, `usb_root/`

## Discrepancy with the handoff

Handoff: "reads the kernel and application files from the FAT32 file system". Code: `include_bytes!("kernel.bin")` / `include_bytes!("app.bin")` (`bootloader/src/main.rs:20-21`); everything is baked into `BOOTX64.EFI` (16,896 bytes). On disk, `usb_root/EFI/BOOT/` contains only the EFI.

Consequences of the current scheme: any kernel change requires rebuilding the bootloader; `cargo build` in `bootloader/` does not build on a clean clone until the `.bin` files exist (they are in `.gitignore`).

## Plan

1. In the bootloader: `boot_services.get_image_file_system(image_handle)` → `open_volume()` → `open("\\EFI\\MIND\\kernel.elf", READ)` → `RegularFile` → `get_info::<FileInfo>()` for the size → `allocate_pages` → `read`. Likewise for `app.elf`.
2. `02_build.sh`: copy the artifacts to `usb_root/EFI/MIND/` (or to the root), not to `bootloader/src/`.
3. Print a "file not found" error via `stdout()` before `exit_boot_services` and do `stall` + `reset`, rather than `unwrap` into `loop {}`.
4. Optional: the file path via `LoadOptions` or an `EFI\MIND\boot.cfg` config.

## Acceptance criteria

- `BOOTX64.EFI` does not contain `include_bytes!`; the EFI size does not depend on the kernel.
- Replacing `kernel.elf` on disk without rebuilding the bootloader changes the behaviour in QEMU.
- A missing file produces a readable on-screen message.

## Related

Naturally done together with [002](002-elf-loader.md): read ELF directly.
