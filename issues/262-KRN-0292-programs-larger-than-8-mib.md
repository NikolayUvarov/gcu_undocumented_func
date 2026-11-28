# 262-KRN-0292 — Programs larger than 8 MiB: `IMAGE_MAX` 512 MiB (ABI 40)

**Type:** kernel, loader, ABI · **Owner:** `KRN` (the kernel session) · **Priority:** P1 · **Status:** in progress (step 1 made, 2026-11-28) · **Blocked by:** — · **Main task:** [262](262-model-runtime-on-the-machines-full-hardware.md), and [264](264-ladybird-on-mind-core-a-port-kept-outside-the-os.md) · **Constitution:** MC-3.3, MC-3.13, MC-5.1, MC-12.4

## Problem

**APP's request of 2026-11-27**, at the maintainer's word, recorded in `requests-KRN.md`.
- The kernel refused an image whose loaded segments span more than 8 MiB ("INVALID ELF IMAGE OR ENTRY POINT").
- The loader refused a program file larger than 4 MiB.
- A task's stack started 16 MiB above its image's start, so no image could grow past 16 MiB without moving the windows above it.
- APP measured Ladybird's `WebContent` for amd64 at 94 MB stripped (`.text` 75.7 MB). An inference runtime with kernels for several CPUs and accelerators grows past 8 MiB too.

## Plan

**Step 1 (this commit): one bound for every program, `IMAGE_MAX` 512 MiB, in `common/abi.rs` (ABI 40).**
- APP asked for "some hundreds of MiB", or a bound per program. The kernel track takes one bound: a program's actual use is bounded by the memory quota its spawner gives, which the owner already sets per program (`data/memory.conf`).
- **The kernel** takes an image whose loaded segments span up to `IMAGE_MAX`, a large `.bss` included. Past it: `ELF IMAGE ABOVE 512 MIB`, which `SPAWN` answers with `ERR_INVALID`.
- **The loader** reads a program file up to `IMAGE_MAX` (4 MiB before).
- **The windows.** A task's image window is `IMAGE_MAX` long. The stack, screen, information page, mailbox, exit page and heap lie above it, as before but 496 MiB higher. An unmapped page stays below the stack, and the heap window keeps more than 510 GiB.
- **The charge** is as before: the image's pages go to the spawner's memory quota and every payer above it (issue 168).

**What step 1 leaves as it was. These are stated, not promised.**
- **One run of memory, twice.** The image is copied into one contiguous run of the frame pool. The loader reads the file into one heap block, which is one contiguous run too. So a 94 MB program needs two runs of about 94 MB free when it starts, and a program with workers (`SPAWN_WORKERS`) keeps a third, its file's copy.
- **No sharing.** Instances of one program share no pages: each gets its own copy of the image. A browser with a content process per site pays the image once for each.
- **The request note's sections.** The loader reads at most 64 section headers when it looks for `.mind_request`. A program with more sections is started as one that asks for nothing.

**Step 2, when 264 or a native inference runtime needs it** (asked as "later" by APP):
- the image in frames of their own (no contiguous run), and read-only segments shared between instances of one file;
- `PT_TLS`, and threads in one address space with a wait on a memory word;
- an application starting the helper programs its own package names.

## Acceptance criteria

1. `common/abi.rs` at ABI 40 with `IMAGE_MAX`. `tests/runtime.rs` pins (48, 40). It also checks three things: `app.elf` grown to just under 512 MiB loads with its `.bss` zeroed; past 512 MiB it is refused with `ELF IMAGE ABOVE 512 MIB`; and the windows lie in order above the image. **Met on the branch** (48 host tests pass).
2. The `memory` suite's `large_image_check`. `bigclock`, `clock.elf` whose image spans 72 MiB from a file of 6 MiB, starts and runs, and `stat` shows its image of 72 MiB. `hugeclock`, past `IMAGE_MAX`, is refused with nothing left allocated. **To be run** on x86, the one architecture whose CI runs the `memory` suite. aarch64's groups run every other program with the moved windows.
3. Every QEMU suite on both architectures passes with the windows moved: the landing gate.

## Related

[262](262-model-runtime-on-the-machines-full-hardware.md), [264](264-ladybird-on-mind-core-a-port-kept-outside-the-os.md), [262-KRN-0164](../issues-done/262-KRN-0164-stacks-larger-than-64-kib.done) (the stack's bound), [262-KRN-0163](../issues-done/262-KRN-0163-workers-on-several-cpus.done) (workers), issue 168 (the charge).
