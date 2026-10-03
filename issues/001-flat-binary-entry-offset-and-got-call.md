# 001 — Flat binaries: `_start` is not at offset 0, `memset` is called at address 0

**Type:** bug · **Priority:** critical · **Status:** open
**Affects:** `kernel/linker.ld`, `app/linker.ld`, `*/.cargo/config.toml`, `02_build.sh`

## Problem

The handoff describes the pipeline as "a dump of the `.text/.rodata/.data` sections", but `02_build.sh` invokes `llvm-objcopy -O binary` without `-j`, and `linker.ld` does not control the auxiliary PIE sections. As a result (confirmed with `readelf`/`xxd`, details in [knowledge/03](../knowledge/03-flat-binary-layout-analysis.md)):

1. In the kernel, `.dynsym/.gnu.hash/.hash/.dynstr/.rela.dyn` are placed **before** `.text`; `_start` is at offset `0x60`, while the loader jumps to `kernel_addr + 0`. 96 bytes of hash tables get executed. It only works if `rax` happens to contain an aligned address.
2. `core::ptr::write_bytes` compiles to `call *memset@GOT(%rip)`; the GOT slot in `.bin` contains `0`, and nobody applies the `R_X86_64_RELATIVE` relocation. The call goes to physical address `0`. The same applies to app.

## Acceptance criteria

- `readelf -h kernel` and `readelf -h app` show `Entry point address: 0x0`.
- `readelf -r` for both: `There are no relocations in this file`.
- `llvm-objdump -d` contains no `callq *...(%rip)` into a `.got` address.
- `xxd -l 4 kernel.bin` starts with the code of `_start` (`55 41 57 41` for the current code).
- Running in QEMU: a pulsing circle; on space — a square.

## Verified solution (built in the scratchpad, not run in QEMU)

1. `.cargo/config.toml` (both crates):
   ```toml
   rustflags = ["-C", "link-arg=-Tlinker.ld", "-C", "relocation-model=pic", "-Z", "relax-elf-relocations=yes"]
   ```
   The flag is nightly-only; the project is on nightly anyway.
2. `linker.ld` (both crates) — add after `.bss`:
   ```ld
   .dynsym   : { *(.dynsym) }
   .gnu.hash : { *(.gnu.hash) }
   .hash     : { *(.hash) }
   .dynstr   : { *(.dynstr) }
   .rela.dyn : { *(.rela.dyn) *(.rela.*) }
   .dynamic  : { *(.dynamic) }
   .got      : { *(.got .got.*) }
   /DISCARD/ : { *(.eh_frame) *(.comment) }
   ```
3. `02_build.sh`:
   ```bash
   $OBJCOPY -O binary -j .text -j .rodata -j .data kernel/target/.../kernel bootloader/src/kernel.bin
   $OBJCOPY -O binary -j .text -j .rodata -j .data app/target/.../app       bootloader/src/app.bin
   ```
4. Add a post-build self-check to `02_build.sh`: `readelf -h ... | grep -q 'Entry point address: *0x0'` and `readelf -r ... | grep -q 'no relocations'`, otherwise `exit 1`.

Expected sizes after the fix: `kernel.bin` ≈ 565 bytes, `app.bin` ≈ 808 bytes.

## Related

- Fully superseded by issue [002](002-elf-loader.md) (the ELF loader applies relocations itself), but until then this fix is needed for stability.
