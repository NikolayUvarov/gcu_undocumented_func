# 215-PRT-0017 — `arch/wasm`: the kernel as a module the page drives, its tasks on the A64 emulator

**Type:** kernel (`kernel/src/arch/wasm`), the page (`web/`) · **Owner:** `PRT` (the kernel session) · **Priority:** P1 · **Status:** in progress (2026-11-28) · **Blocked by:** [215-PRT-0016](215-PRT-0016-an-a64-user-mode-emulator.md) (the emulator, in the landing queue) · **Main task:** [215](215-mind-core-in-a-browser-tab.md), step 1 · **Constitution:** MC-12.1, MC-12.3

## Problem

Step 1 of [215](215-mind-core-in-a-browser-tab.md): the kernel built for WebAssembly, so that init, the services and the shell run in Node and then in a tab. The way is B of [docs/wasm/README.md](../docs/wasm/README.md), section 4.1.

This task is the kernel's half and the page's start:
- `arch/wasm`;
- the boot from what the page lays out;
- the console.

The desktop on a canvas (the framebuffer, keys and pointer) follows it as its own task.

## Plan

1. **`kernel/src/arch/wasm`**, beside `aarch64` and `x86_64`, with the same modules:
   - **`context`:** aarch64's frame, word for word, so the scheduler saves and resumes tasks unchanged.
   - **`mmu`:** aarch64's descriptors, so `paging.rs` builds the same spaces. The emulator walks them. The kernel needs no identity map: the module's memory is its physical memory.
   - **`run`:** the page's calls.
     - `mind_start` builds `BootInfo` from words the page wrote, and boots to init.
     - `mind_turn` resumes the frame the scheduler chose. A task runs on `a64` until it calls, faults or an interrupt is due. Its exit then becomes the ESR, FAR and kind an exception entry would give, and `scheduler::interrupt` takes it.
     - The idle loop returns the page its thread, with the time to the next tick.
   - **`interrupts`:** a 10 ms tick on the page's clock, and lines the page raises.
   - **`serial`:** the page's console as the kernel's console channel (PLATFORM_CONSOLE), on line 1.
   - **The rest have nothing to drive:** no ACPI, PCI, watchdog, ports or TPM.
2. **The generic kernel:** `boot` taken out of `_start`, the aarch64 ELF machine and log mirror for wasm too, and `efiapi` and the kernel's end symbol kept off wasm. Nothing changes on x86 or aarch64.
3. **The build:**
   - `wasm64-unknown-unknown` with `build-std`, so that `usize` and pointers are 64 bits as in the aarch64 programs' `BootInfo`;
   - then Binaryen's `--memory64-lowering --table64-lowering` to wasm32, which every browser and Node 22 runs.
4. **The page, `web/`:**
   - `mind.js`, the same in a tab and in Node. It lays the memory out above the module's heap base: the screen, the kernel's heap, the ELF files of `BOOT_FILES`, the boot volume as a live boot's (an ISO's FAT image) and the frame pool. It then starts the kernel and runs its turns.
   - `node/run.mjs` runs it with the console on stdout and a script of keys.

## Acceptance criteria

- [x] The kernel builds for `wasm64-unknown-unknown` with no warning, and lowers to wasm32 (307 KB) with four imports from the page.
- [ ] In Node 22, from the aarch64 build of this tree: the kernel boots, init starts the services, and the shell's prompt answers `uptime`, `ps` and `ls`. The boot's time to the prompt is recorded.
- [ ] The x86 and aarch64 kernels still build, and their boot groups pass (the gate).
- [ ] `docs/wasm/README.md` says what runs and how to run it. `docs/profile/wasm.md` names the TCB (the browser or Node, the page's JS, the emulator) and what is not there (MC-12.3).

## Related

- [215](215-mind-core-in-a-browser-tab.md), [215-PRT-0016](215-PRT-0016-an-a64-user-mode-emulator.md), [docs/wasm/README.md](../docs/wasm/README.md).
