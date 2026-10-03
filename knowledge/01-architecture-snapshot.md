# Architecture snapshot (from the code, commit 8ad7550)

## Components

| Component | Path | Target | Format | Artifact size |
|-----------|------|--------|--------|------------------|
| Bootloader | `bootloader/` | `x86_64-unknown-uefi` | PE/COFF (`BOOTX64.EFI`) | 16 896 bytes |
| Kernel | `kernel/` | `x86_64-unknown-none` | flat binary `kernel.bin` | 896 bytes |
| App (userspace) | `app/` | `x86_64-unknown-none` | flat binary `app.bin` | 1 136 bytes |

Three independent cargo packages with no workspace. There is no shared crate for types.

## The `BootInfo` contract

The struct is duplicated three times (bootloader/kernel/app), `#[repr(C)]`, identical:

```rust
pub struct BootInfo {
    pub fb_ptr: *mut u8,   // GOP framebuffer address
    pub width: usize,
    pub height: usize,
    pub stride: usize,     // in pixels, not bytes
    pub app_ptr: *const u8 // physical address of the loaded app.bin
}
```

Calling convention: `extern "sysv64" fn(&BootInfo) -> !`. The pointer to `BootInfo` lives on the bootloader's stack and stays valid forever, since the bootloader never returns.

Risk: duplication without a single source of truth — if the fields change in one place, the contract breaks silently (see [05-observations-and-risks.md](05-observations-and-risks.md)).

## Control flow

1. `bootloader/src/main.rs:20-21` — `kernel.bin` and `app.bin` are embedded in the EFI via `include_bytes!` (not read from FAT32).
2. `:27-33` — `allocate_pages(AnyPages, LOADER_DATA, len/4096+1)` for each binary, `copy_nonoverlapping`.
3. `:35-37` — GOP: `get_handle_for_protocol` + `open_protocol_exclusive`; the current mode is taken without selecting one and without checking `PixelFormat`.
4. `:53` — `exit_boot_services(LOADER_DATA)`; the memory map is discarded.
5. `:56-57` — `transmute(kernel_addr)` and a jump to offset **0** of the flat binary (see [03](03-flat-binary-layout-analysis.md): in the current build `_start` is not located there).
6. `kernel/src/main.rs:18-62` — infinite loop: polls port `0x64`, and when bit 0 is set reads `0x60`; scancode `0x39` (space) → `transmute(info.app_ptr)` and a call into the application. Otherwise a pulsing circle is drawn; the delay is `1_000_000` `nop`s.
7. `app/src/main.rs:20-54` — infinite loop: a square rotating via a sine lookup table. It never returns to the kernel.

## What is not in the code (although the README/handoff mention it)

- IDT, PIC remapping, IRQ0/IRQ1 handlers, context switching, `int 0x80` — entirely absent.
- Global allocator, `alloc`, `format!` — absent.
- Reading files from FAT32 via `SimpleFileSystem` — absent.
- Font/primitive rendering — absent (only a circle and a square, pixel by pixel).
- Handing control to userspace on a timeout — absent (only on a key press).
- A dedicated kernel stack, `.bss` zeroing, memory map handling — absent.

All of this is filed as issues in `issues/`, see [04-handoff-vs-code-matrix.md](04-handoff-vs-code-matrix.md).
