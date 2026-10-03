# Analysis of the ELF and flat-binary layout

All data was obtained from the current build (`./02_build.sh`, 2026-09-17). Commands: `readelf -S/-l/-r/-s`, `llvm-objdump -d`, `xxd`.

## Finding 1 (critical): the kernel's `_start` is at offset 0x60, but the bootloader jumps to 0

`readelf -S kernel`:
```
[ 1] .dynsym   0x000000  size 0x18
[ 2] .gnu.hash 0x000018  size 0x1c
[ 3] .hash     0x000034  size 0x10
[ 4] .dynstr   0x000044  size 0x01
[ 5] .rela.dyn 0x000048  size 0x18
[ 6] .text     0x000060  size 0x235   <-- _start is here
[ 7] .dynamic  0x000298
[ 8] .got      0x000378  size 0x08
Entry point 0x60
```

`xxd -l 0x60 kernel.bin` — the first 96 bytes: zeros, `01 00 00 00 ...`, `78 03 ...`, `50 02 ...` (the contents of the hash tables and the RELA entry). This is what gets executed first after `transmute(kernel_addr)` in `bootloader/src/main.rs:56`.

Why app is different: app has a `.rodata`, and LLD places orphan sections with the `A` flag next to it (after `.text`); in the kernel `.rodata` is empty, and the read-only-rank orphan sections ended up before `.text` (their rank is lower than that of `AX`). The app entry is 0x0 — correct.

Why this may have "worked": zeros decode as `add [rax], al`. If `rax` holds `kernel_addr` at the moment of the jump (page-aligned → `al = 0`), the instructions change nothing and execution "slides" down to `_start`. This depends on the incidental contents of a register; it is not a correct start. Any change to the bootloader code or to section sizes will change the behavior.

## Finding 2 (critical): the `memset` call goes through the GOT, and the GOT slot in the flat binary is zero

`llvm-objdump -d kernel`:
```
13b:  callq *0x237(%rip)   # 0x378  (.got)
```
`readelf -r`: a single `R_X86_64_RELATIVE` relocation at address `0x378` with addend `0x250` (= the address of `memset`).
`xxd -s 0x378 -l 8 kernel.bin` → `00 00 00 00 00 00 00 00`.

By default LLD does not write the addend into the relocation site (`--no-apply-dynamic-relocs`), and nobody applies relocations at runtime. Result: `core::ptr::write_bytes(fb_ptr, 0, ...)` in `kernel/src/main.rs:38` is a `call` to **physical address 0**. The same applies to app: `callq *0x3c6(%rip) # 0x468`, the slot is zero.

The reason the call goes through the GOT at all: `memset` is a hidden local symbol from `compiler_builtins`, but the relocation in the object file is a non-relaxable `R_X86_64_GOTPCREL`, because `relax-elf-relocations` is disabled for the `x86_64-unknown-none` target.

## Verified fix (experiment in a scratchpad, not applied to the project)

1. The `-Z relax-elf-relocations=yes` flag → `addr32 callq 0x240 <memset>`, the `.got` section is empty, `.rela.dyn` is absent.
2. In `linker.ld`, explicitly list the auxiliary sections **after** `.bss`:
   ```ld
   .dynsym : { *(.dynsym) }  .gnu.hash : { *(.gnu.hash) }  .hash : { *(.hash) }
   .dynstr : { *(.dynstr) }  .rela.dyn : { *(.rela.dyn) *(.rela.*) }
   .dynamic : { *(.dynamic) } .got : { *(.got .got.*) }
   /DISCARD/ : { *(.eh_frame) *(.comment) }
   ```
3. In `02_build.sh`, dump only the useful sections: `llvm-objcopy -O binary -j .text -j .rodata -j .data`.

Result: kernel — `Entry 0x0`, `There are no relocations in this file`, `kernel.bin` = 565 bytes, starting with `55 41 57 41` (`push rbp; push r15`). app — `Entry 0x0`, `app.bin` = 808 bytes, `SIN_TABLE` is addressed via `lea 0x138(%rip)`.

An alternative without the `-Z` flag: `-C relocation-model=static` + `--no-pie` removes the relocations, but the GOT call remains with the absolute address `0x1f0` (verified) — suitable only when loading at a fixed address (`AllocateType::Address`). Not recommended.

Filed as issue 001. Running in QEMU after the fix is a mandatory check (QEMU is not available here).

## Finding 3: `.bss` in the flat binary

`llvm-objcopy -O binary` does not write a trailing NOBITS section. Even with `.bss` declared in `linker.ld`:
- pages are allocated from `kernel_bytes.len()` — there may be no room for `.bss`;
- memory from `allocate_pages` is not guaranteed to be zeroed — statics will get garbage.

For `static mut`/an allocator, either an ELF loader is needed (`p_memsz > p_filesz` → zeroing), or, in the flat variant: `__bss_start/__bss_end` symbols in `linker.ld`, passing the size to the bootloader, allocating `memsz` pages and `write_bytes(0)`. → issue 002/003.

## What is fine in the layout

- `.text._start` really is first in `.text` (verified with the disassembler).
- Accesses to `.rodata` are RIP-relative — PIC works, and the binaries load at any address.
- Stack: the kernel and app run on the UEFI bootloader's stack (it remains valid after `exit_boot_services`). There is no dedicated stack — one will be needed for multitasking.
