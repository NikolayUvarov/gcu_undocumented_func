# 215-PRT-0016 — An emulator of user-mode A64 for the browser's kernel (step 0 of 215)

**Type:** porting, a new crate (`a64`) · **Owner:** `PRT` (the kernel session) · **Priority:** P1 · **Status:** in progress (made on its branch, 2026-11-28; to land) · **Blocked by:** — · **Main task:** [215](215-mind-core-in-a-browser-tab.md) · **Constitution:** MC-3.3, MC-12.1, MC-12.3

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

## Progress (2026-11-28, branch `claude/215-PRT-0016-a64-user-mode-emulator`)

**Made:**
- `a64/`: the crate (`src/exec.rs`, the instructions; `src/mmu.rs`, the walk and the TLB). No dependencies; it builds for the host, `wasm32-unknown-unknown` and `wasm64-unknown-unknown` (with `-Z build-std=core`), with no warning.
- `a64/node/`: a stand-in for the kernel compiled to wasm, and `run.mjs`.
  - It loads a MIND Core static PIE at aarch64's USER_IMAGE and applies its `R_AARCH64_RELATIVE` relocations, as the kernel's loader does.
  - It answers LOG, UPTIME, EXIT, ALLOC and FREE, and refuses every other system call, naming it.
  - `tests/layout.rs` checks its offsets and numbers against `common/abi.rs`.
- `scripts/a64_census.py`, and the crate's tests in `scripts/host_tests.sh`.

**Criterion by criterion:**
1. **Met:** the host, wasm32 and wasm64 builds above.
2. **Met:** `tests/instructions.rs`, 12 tests:
   - `svc`'s and `wfi`'s PC past themselves; `brk`, an undefined instruction and a fault leaving PC and the registers unchanged;
   - the budget, a misaligned PC, the exclusive monitor and its clearing at each run, RNDR with and without entropy;
   - the walk: a user page mapped and run through; a read-only page written; a kernel page read; a UXN page fetched; an unmapped page; the access flag clear; an address above 48 bits; a device's register;
   - a write across two pages changing nothing when the second refuses; a changed table seen only after a flush.
3. **Met another way, stated here.** No reference emulator was installed in this session. Instead, the same Rust work is computed natively and, built for `aarch64-unknown-none-softfloat` as MIND Core's programs are, on the emulator (`tests/native.rs`, `tests/programs/work.rs`). It covers:
   - SHA-256, 64-, 32- and 128-bit division and products, sorting, bit operations, the flags, soft-float, `core::fmt`, atomics, copies, sign-extending loads, a state machine, narrow wrapping arithmetic.
   - Its 16 results agree over 1,383,784 instructions.
   - Changed on purpose, REV made wrong by one bit changed slot 5 alone, and a wrong UMULH ended the program early; both failed the test.
4. **Met:**
   - `uptime`, built from `main` for aarch64, ran in Node on the wasm build to its end in 6,583 instructions. It passed `check_abi` (ABI 40), took its heap through ALLOC, was refused CAP_INFO (29) and MEM_SHARE (15), and printed `uptime: no access to sysmon (start it from the shell)`.
   - The differential program in Node gave the same 16 results as the native build, at 47–51 million instructions a second warm (this container's CPU; the host's own test build: 82–90 million).

## Related

[215](215-mind-core-in-a-browser-tab.md), [docs/wasm/README.md](../docs/wasm/README.md), [205](205-aarch64-boards.md).
