# 215-PRT-0016 — An emulator of user-mode A64 for the browser's kernel (step 0 of 215)

**Type:** porting, a new crate (`a64`) · **Owner:** `PRT` (the kernel session) · **Priority:** P1 · **Status:** in progress (2026-11-28) · **Blocked by:** — · **Main task:** [215](215-mind-core-in-a-browser-tab.md) · **Constitution:** MC-3.3, MC-12.1, MC-12.3

## Problem

- In a browser tab, MIND Core's kernel is compiled to wasm (215, way B), but its programs stay the aarch64 build's own files.
- Something must run their instructions inside the kernel's wasm, with their memory translated through the kernel's own page tables, so that isolation stays the kernel's.
- unkernel's emulator (kwemu) is GPL and written in Zig; none of it can come here. This crate is written for MIND Core, under MIT or Apache-2.0.

**What it must run.** Counted over the aarch64 build of 2026-11-10 (112 ELF files, 3.36 million instruction words; `scripts/a64_census.py`):

| Class | Words |
|---|---|
| integer loads and stores | 31.6 % |
| data processing, immediate | 26.8 % |
| data processing, register | 20.9 % |
| branches | 18.5 % |
| NOP | 1.35 % |
| exclusive and ordered loads and stores (`ldxr`, `stxr`, `ldar`, `stlr`) | 0.32 % |
| `isb`, `svc`, `dmb`, `brk` | under 0.6 % together |

- **System registers read or written by programs:** NZCV (`tls`), RNDR (`libmind`'s entropy) and CNTVCT (one fixture).
- **None in any program:** LSE atomics, SVE, and FP or SIMD, except 8 FP instructions in one test fixture, `busy_app`.
- Every other system register is the kernel's own, which runs as wasm, not on the emulator.

## Plan

1. **A crate `a64`, no_std, with no allocation:**
   - `Cpu`: X0–X30, SP_EL0, PC, NZCV, TPIDR_EL0, and the exclusive monitor;
   - `run(cpu, memory, budget)` runs until an exit: `svc` with its immediate, a fault (the address and whether it read, wrote or fetched), an undefined instruction with its word, `brk`, `wfi`/`wfe`, or the end of the budget;
   - `isb`, `dmb`, `dsb` and the hints are no-ops: one CPU, no reordering.
2. **The instructions:** the base A64 integer set the census found: data processing on immediates and registers, every load and store form in it, branches, the exclusives with the monitor, and `mrs`/`msr` of NZCV, TPIDR_EL0, RNDR (from the page's entropy) and CNTVCT (from the page's clock). Anything else is an undefined instruction, reported with its word, never run half-way.
3. **Memory through the kernel's page tables:**
   - a stage-1 walk of the aarch64 format: 4 KiB granule, 48-bit addresses, four levels;
   - the user's permissions (AP, UXN) checked on every access;
   - a small TLB in software that the kernel flushes when it changes a table;
   - a page that is not RAM calls the device model.
4. **Its tests:**
   - host tests for each class, from the Arm ARM's definitions;
   - a differential run of generated instruction sequences against a reference emulator on the host. This is a development tool only, not a dependency of the build or of CI;
   - the census script, kept in `scripts/`.
5. **A first program:** a MIND test program built for aarch64, loaded at its link address and run in Node (the crate compiled to wasm) up to its system calls, each mailbox request printed. The instructions a second in Node are measured and stated.

## Acceptance criteria

1. `a64` builds for the host, for `wasm32-unknown-unknown` and for `wasm64-unknown-unknown`, with no warning.
2. Host tests cover every class above, and the page-table walk's faults (unmapped, read-only written, a kernel page, UXN fetched).
3. The differential run agrees on a stated number of generated sequences.
4. A MIND aarch64 test program runs in Node to its first system calls; its instructions a second are measured and recorded here.

## Related

[215](215-mind-core-in-a-browser-tab.md), [docs/wasm/README.md](../docs/wasm/README.md), [205](205-aarch64-boards.md).
