# 262-KRN-0161 — Large pages for a program's large blocks

**Type:** kernel (generic page tables, `arch/*/mmu.rs`) · **Owner:** kernel session · **Priority:** P2 · **Status:** open (in `main` a2a1062e; the machines' measurements left) · **Blocked by:** — · **Main task:** [262](262-model-runtime-on-the-machines-full-hardware.md) · **Constitution:** MC-3.13, MC-5.1, MC-12.1

From the tools track's request of 2026-10-23 in [requests-KRN.md](requests-KRN.md), item 2.

## Problem

- **Every user page is 4 KiB.** A model's weights are read once per token, across gigabytes.
- **The cost.** With 4 KiB pages each 2 MiB of them takes 512 TLB entries and a page table of its own, so the walk misses the TLB constantly, and the tables take 2 MB per GiB.

## Plan

1. **2 MiB pages.** A part of a block that is 2 MiB-aligned in both its virtual and its physical address is mapped by one entry one level up:
   - x86-64: a page directory entry with `PS`;
   - aarch64 (4 KiB granule): a level-2 block descriptor.
2. **1 GiB pages** where the CPU has them (x86-64: `CPUID.80000001h:EDX.Page1GB`; aarch64: level-1 blocks), for a whole GiB of a block that is 1 GiB-aligned in both its addresses. Otherwise its 2 MiB pages stay.
3. **The walk learns large leaves.** `readable`, `validate_read`, `unmap` and a revocation find a large leaf and stop there.
   - A large leaf is unmapped whole only: blocks are freed whole, so it is never split.
4. **The attributes stay as for 4 KiB pages:** user, write-back, never executable, never global; writable as the block is.
5. **Device memory and framebuffers** keep 4 KiB pages.

## Acceptance criteria

- [x] Host tests: 2 MiB and 1 GiB leaves map, are read and written through, are unmapped whole, and reject an overlap.
- [x] A QEMU suite (x86 and aarch64): a 1 GiB block is mapped with large pages, as the task's kernel bytes with its page tables in `stat` show, and its bytes are written and read back.
- [ ] A random walk over 1 GiB timed with large pages and with 4 KiB pages, by `kbench`, on each machine at hand. Each result is stated for its machine (MC-12.1).

## Progress (2026-10-23)

Made on `claude/youthful-mendel-mf1soy` (ed9dac59), ABI 19:
- **The walk** (`kernel/src/paging.rs`, `find`) stops at a large leaf for `readable`, `writable`, `validate_read`, `regions` (`STAT VMAP`), the overlap check before a map, and `unmap` (a revocation goes through `unmap_leaves`). A large page is cleared whole.
- **`map`** puts memory a task reads and writes in the largest page that lies whole in the range with both its addresses on its boundary. An empty table that `unmap_leaves` kept takes smaller pages instead of being replaced, so no valid entry ever changes kind.
- **`ALLOC_SMALL_PAGES`** (`ALLOC`'s `arg2`) keeps a block in 4 KiB pages, for the comparison; `Pages::new_small` in libmind. Any other bit of `arg2` refuses the call.
- **Where the CPU has 1 GiB pages:** x86-64 `CPUID 80000001h` EDX bit 26, read once at boot; aarch64 always. The hardware report names the sizes.
- **Checked** (this session's container, QEMU TCG):
  - host tests `large_pages_translate_unmap_whole_and_reject_an_overlap` and `large_pages_carry_bytes_and_other_ranges_keep_small_ones`;
  - `big_memory_check` (`--suites bigmem`, and the `normal` suite with more than 4 GiB), 4 CPUs and 6 GiB, x86 and aarch64: the program holding 3 GiB had 63 KiB of kernel bytes (`stat`: its task record, context, info pages and page tables), where its page tables alone would take about 6 MiB in 4 KiB pages; every page written and read back;
  - kbench's `pages` group in the same runs, the median of a read: x86 386 ns in large pages and 528 ns in 4 KiB pages; aarch64 438 ns and 641 ns.
- **What the numbers say, and do not.** Under TCG, QEMU's own translation cache sits between the guest's pages and the host, so the two rows say little of a CPU's TLB. A machine's figure comes from a run on it (MC-12.1).
- **In `main`** a2a1062e (landing round 21, 2026-10-23): the gate `land-a2a1062e` passed every group, x86 and aarch64.
- **Left:** the random read timed on each machine at hand. The suites run QEMU without KVM everywhere, the Effector machines too, so their figures are emulation's; a machine's figure needs QEMU with KVM there, or `kbench pages` on the machine itself (the MacBook Pro by the maintainer).

## Related

[262-KRN-0160](262-KRN-0160-large-private-memory-for-a-named-program.md), [262](262-model-runtime-on-the-machines-full-hardware.md), [176](176-test-and-performance-utilities.md) (`kbench`).
