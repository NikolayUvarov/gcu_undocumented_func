# Other observations and risks

A list of things that do not warrant a separate issue or that supplement existing ones.

## Bootloader

- **The memory map is thrown away.** `exit_boot_services` returns a `MemoryMap`; it is bound to `_memory_map` and not passed to the kernel. Without it the kernel cannot build a physical page allocator. Will be needed for issue 003.
- **`uefi = "=0.27.0"`** — the `SystemTable<Boot>` API is deprecated in newer versions of the crate (0.30+) in favor of the global `uefi::boot::*`. Upgrading will require rewriting the code; while the version is pinned, that is fine.
- **The `wcslen` stub** (`bootloader/src/main.rs:67-77`) is needed by the linker because of `ucs2`; after a `uefi` upgrade it may become unnecessary or, conversely, conflict.
- **The GOP mode is not selected**: the current one is taken (`current_mode_info`). On real hardware this may be 640×480 or a BltOnly buffer without a linear framebuffer. → issue 009.
- **Page allocation** `len/4096 + 1` — correct, but when `len % 4096 == 0` an extra page is wasted; a minor point.
- **No status checks** — everything is `.unwrap()`. If GOP fails, the panic ends up in `loop {}` with no output. For debugging it is worth printing via `system_table.stdout()` before `exit_boot_services`.

## Kernel

- **Stack** — the UEFI bootloader's stack (typically 128 KB in OVMF). No dedicated stack is allocated. Threads/interrupts need their own, plus an IST.
- **Delay via a `nop` loop** — uncalibrated; on a different CPU/QEMU the animation speed changes several-fold. Will go away with the timer (issue 004).
- **Keyboard**: the controller buffer is not flushed and break codes are not filtered out (not a problem yet — we only react to `0x39`).
- **There is no way back from app to the kernel**: `app_entry(info)` is declared `-> !`. This is by design for "Step 0", but the contract should be documented in a comment.
- **`write_bytes` over the whole framebuffer every frame** — at 1920×1080 that is 8 MB through `memset` into MMIO memory without SSE (`+soft-float`, vectorization disabled) — slow, but tolerable for now.

## General

- **`BootInfo` is duplicated** in three crates. It should be moved into a shared `no_std` crate `boot-proto` (without a workspace this can be a path dependency).
- **No workspace** — three separate `Cargo.lock` files, three `target/` directories. A workspace with `per-package-target` (nightly) or a simple root `Makefile`/`justfile` would make life easier.
- **README**: the heading "Iain M. Banks supposed # MIND CORE" is a typo/accidental merge; the OVMF path in the README (`/usr/share/ovmf/OVMF.fd`) is now `/usr/share/OVMF/OVMF_CODE_4M.fd` + `OVMF_VARS_4M.fd` on Debian/Ubuntu. → issue 010.
- **No CI and no tests** — the build is only checked by hand. At a minimum: a GitHub Action that runs `02_build.sh` and checks `Entry point 0x0` via `readelf`.
- **No `CLAUDE.md`/AGENTS instructions** in the repository; this `knowledge/` folder partially fills that role.
