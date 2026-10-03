# Build infrastructure

## Toolchain on the revision machine (2026-09-17)

```
rustc 1.100.0-nightly (574ff7d98 2026-09-14)
cargo 1.100.0-nightly (7941be6fb 2026-09-11)
targets: x86_64-unknown-linux-gnu, x86_64-unknown-none, x86_64-unknown-uefi
llvm-tools-preview: installed (llvm-objcopy, llvm-objdump; llvm-readelf is missing — the system readelf is used)
qemu-system-x86_64: NOT installed in WSL; it is run on Windows via 03_run_qemu_windows.bat
OVMF: OVMF.fd in the project root (4 MB, gitignored); system /usr/share/OVMF/OVMF_CODE_4M.fd
```

Verified: `./02_build.sh` completes without errors and without compiler warnings in all three crates.

## Scripts

| Script | Purpose | Notes |
|--------|-----------|-----------|
| `01_prepare_env.sh` | installs rustup, runs `rustup default nightly` **globally**, adds the targets and llvm-tools | changes the user's default toolchain; no `rust-toolchain.toml` → issue 011 |
| `02_build.sh` | kernel/app → `cargo build --release` → `llvm-objcopy -O binary` → bootloader → copy to `usb_root/EFI/BOOT/BOOTX64.EFI` | objcopy is invoked **without** `-j .text -j .rodata -j .data`, so all alloc sections end up in the `.bin`, including `.dynsym/.hash/.dynamic/.got` → issue 001 |
| `03_run_qemu_windows.bat` | `qemu-system-x86_64.exe -bios OVMF.fd -drive format=raw,file=fat:rw:usb_root -m 512` | there is no Linux variant |

Scripts referenced by the documents but absent from the repository: `patch_008_preemptive.sh` (README), `fix_stable_boot.sh` (handoff). The current one is `02_build.sh`. → issue 010.

## kernel/app compilation flags

`.cargo/config.toml` (identical for kernel and app):
```toml
[build]
target = "x86_64-unknown-none"
rustflags = ["-C", "link-arg=-Tlinker.ld", "-C", "relocation-model=pic"]
```

Facts about the `x86_64-unknown-none` target (from `rustc --print target-spec-json`):
- `position-independent-executables: true`, `static-position-independent-executables: true` — PIE is already the default; the `relocation-model=pic` flag is redundant but harmless.
- `code-model: kernel`, `relro-level: full`, `panic-strategy: abort`, SSE/MMX disabled (`+soft-float`).
- `relax-elf-relocations` is **disabled** by default → LLD does not fold `call *memset@GOTPCREL(%rip)` into a direct `call` → see issue 001.

## `linker.ld` (identical for kernel and app)

Sections `.text` (`.text._start` first), `.rodata`, `.data`, `.bss`, `/DISCARD/ .eh_frame`. The auxiliary PIE sections are not listed, so LLD places them as orphans according to its own rank rules — in the kernel, **before** `.text` (see [03](03-flat-binary-layout-analysis.md)).

The handoff claims there were attempts to include `.bss` "without a linker script" — in the current code `linker.ld` already exists and declares `.bss`. But `llvm-objcopy -O binary` does not write a NOBITS section at the end of the file, and `allocate_pages` counts pages from the file size, so `.bss` is still neither allocated nor zeroed → issue 003.

## Git

- `*.bin`, `usb_root/`, `OVMF.fd`, `Cargo.lock` are in `.gitignore`. Consequence: `cargo build` in `bootloader/` on a clean clone fails on `include_bytes!("kernel.bin")` until kernel/app have been built. The order in `02_build.sh` accounts for this.
- `Cargo.lock` is not committed — for binary crates this reduces reproducibility (uefi is pinned to `=0.27.0`, but transitive dependencies float) → issue 011.
