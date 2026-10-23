# 262-KRN-0161 — Large pages for a program's large blocks

**Type:** kernel (generic page tables, `arch/*/mmu.rs`) · **Owner:** kernel session · **Priority:** P2 · **Status:** open · **Blocked by:** [262-KRN-0160](262-KRN-0160-large-private-memory-for-a-named-program.md) (blocks of 2 MiB runs) · **Main task:** [262](262-model-runtime-on-the-machines-full-hardware.md) · **Constitution:** MC-3.13, MC-5.1, MC-12.1

From the tools track's request of 2026-10-23 in [requests-KRN.md](requests-KRN.md), item 2.

## Problem

- **Every user page is 4 KiB.** A model's weights are read once per token, across gigabytes.
- **The cost.** With 4 KiB pages each 2 MiB of them takes 512 TLB entries and a page table of its own, so the walk misses the TLB constantly, and the tables take 2 MB per GiB.

## Plan

1. **2 MiB pages.** A run of a block that is 2 MiB-aligned in both its virtual and its physical address is mapped by one entry one level up:
   - x86-64: a page directory entry with `PS`;
   - aarch64 (4 KiB granule): a level-2 block descriptor.
2. **1 GiB pages** where the CPU has them (x86-64: `CPUID.80000001h:EDX.Page1GB`; aarch64: level-1 blocks), for a whole GiB of a block whose frames are one aligned GiB of the pool. Otherwise its 2 MiB pages stay.
3. **The walk learns large leaves.** `readable`, `validate_read`, `unmap` and a revocation find a large leaf and stop there.
   - A large leaf is unmapped whole only: blocks are freed whole, so it is never split.
4. **The attributes stay as for 4 KiB pages:** user, write-back, writable and never executable, never global.
5. **Device memory and framebuffers** keep 4 KiB pages.

## Acceptance criteria

- [ ] Host tests: 2 MiB and 1 GiB leaves map, are read and written through, are unmapped whole, and reject an overlap.
- [ ] A QEMU suite (x86 and aarch64): a 1 GiB block is mapped with large pages, as the task's page-table count in `ps` shows, and its bytes are written and read back.
- [ ] A random walk over 1 GiB timed with large pages and with 4 KiB pages, by `kbench`, on each machine at hand. Each result is stated for its machine (MC-12.1).

## Related

[262-KRN-0160](262-KRN-0160-large-private-memory-for-a-named-program.md), [262](262-model-runtime-on-the-machines-full-hardware.md), [176](176-test-and-performance-utilities.md) (`kbench`).
