# MIND Core in a browser tab: a plan

**Status:** a proposal of the kernel track, 2026-11-28, at the maintainer's request. Nothing in it is made yet; what it says MIND Core would do in a browser is a plan, not a guarantee (MC-12.3). Its tasks get numbers when the maintainer chooses the way and the track (section 8).

**The maintainer's request (2026-11-28):** «изучить как сделать следующее решение — запустить mind-core-os в webassembly, аналогично проекту unkernel.dev. Создай план и оцени реалистичность.»

## 1. What unkernel does

[unkernel](https://github.com/pithings/unkernel) runs Linux in a browser tab. Read from its README and its `.agents/kernel.md` and `.agents/status.md`, 2026-11-28:

- **The kernel is compiled to wasm.** It is a new architecture, `arch/wasm`, on Linux 7.3.4, grown from Joel Severin's linux-wasm port. It is built for wasm64 and lowered to wasm32 by Binaryen.
- **User programs are unchanged aarch64 Linux binaries.** An aarch64 CPU written in Zig (kwemu) is linked into the kernel. It runs them by an interpreter of decoded blocks and a JIT to wasm modules.
- **The MMU is real.** "kwemu walks the kernel's own page tables, so memory management is done by the real Linux code."
- **Task switches use JSPI** (`WebAssembly.Suspending`), because Linux keeps a kernel stack for every task.
- **Devices are the page's.** A framebuffer, input, sound and block devices are its own (`kwfb`, `kwinput`, `kwsnd`, `kwblk`). The disk is a read-only image fetched by ranges, with changes kept in IndexedDB. The network is `kwnet` with a TCP/IP stack in JS and a Wisp proxy, since a page cannot open TCP.
- **Its figures, its own, measured under Node:**
  - it reaches `/init` about 70 ms after it starts, and the page downloads about 10 MB;
  - the interpreter runs about 250–300 million guest instructions a second, and the JIT 1.6–2.3 times that.
- **Browsers:** Chrome and Edge 137+, Firefox 153+, Safari 27+. They are the ones with JSPI.
- **Licence:** GPL-2.0-or-later, the SDK MIT. No code of it can come into MIND Core (MIT/Apache-2.0). This plan takes its published design as a reference and nothing else.

## 2. Three ways for MIND Core

| | A. The whole machine emulated | B. unkernel's way | C. Programs compiled to wasm |
|---|---|---|---|
| What runs in the tab | QEMU compiled to wasm, running the aarch64 `virt` image as it is | MIND Core's kernel compiled to wasm; the aarch64 programs as they are, on an emulator of user-mode A64 | the kernel and every program compiled to wasm, one wasm instance per task |
| Changes to MIND Core | none | a new `arch/wasm` in the kernel, an emulator, the page's devices | a third set of every program, and a new model of memory |
| Speed | slow: full-system emulation, with no JIT of our own | an interpreter's speed for user code, the kernel's own at native wasm speed | near native |
| Isolation | QEMU's MMU | our page tables, checked by our emulator | the wasm sandbox; `MEM_SHARE` cannot alias pages between instances |
| Risk | that QEMU-in-wasm boots our image with a display at all; untried | the emulator's correctness and speed | `MEM_SHARE`, the screens and every ring that two tasks share would need copying or a new design |

**The kernel track proposes B.**
- A is at best a teaser of a few days: slow, and not unkernel's kind.
- C changes the system's semantics. MIND Core's isolation and its shared pages rest on page tables, which wasm instances do not have.

## 3. Why B fits MIND Core better than it fits Linux

Measured in this repository, `main` of 2026-11-28:

- **No kernel stack per task, so no JSPI.**
  - An exception pushes its frame on the CPU's stack, and the handler returns the frame to resume (`kernel/src/arch/aarch64/context.rs`).
  - So the kernel's loop in wasm is: pick a task, run its user code on the emulator until a system call, a fault or the end of its slice, handle that, repeat.
  - No stack is ever suspended, so it needs no JSPI, and it runs in browsers older than the ones unkernel needs.
- **User code is integer only.** Every aarch64 program is built for `aarch64-unknown-none-softfloat` (`scripts/build_aarch64.sh`). The emulator needs the base integer A64 instructions with the exclusive loads and stores; it needs no FP or AdvSIMD. This is to be confirmed over every binary of the image in phase 0.
- **A small kernel with a small architecture layer.**
  - The kernel is about 9,800 lines of Rust.
  - `arch/aarch64` is 1,738 of them and `arch/x86_64` 2,618.
  - The rest names an architecture in 12 places, in 5 files.
- **A system call is one instruction.** A program writes its request into its mailbox page and executes `svc #0` (`libmind/src/arch/aarch64.rs`). The emulator stops on it and the kernel reads the mailbox, as on a real machine.
- **The drivers can stay as they are.**
  - `virtio_blk`, `virtio_net` and `virtio_input` drive virtio-pci devices through mapped BARs.
  - If the page offers the same devices (a PCI configuration space and modern virtio-pci devices, in JS), the same driver binaries run.
  - The boot framebuffer is a range of memory the page draws on its canvas.
- **One image.** The programs are the aarch64 build's own files; only the kernel is another.

## 4. The design of B

**4.1 The kernel's `arch/wasm`** (KRN):
- **The build:** for `wasm64-unknown-unknown`, from the pinned nightly with `build-std`, so that `usize` stays 64 bits: `BootInfo` and the mailbox, which the aarch64 programs read, are laid out with it. Binaryen lowers it to wasm32 for a browser without memory64, as unkernel does.
- **The boot:**
  - the page builds `BootInfo` (the framebuffer, the programs, the memory) as the UEFI loader does;
  - the kernel's linear memory is the machine's physical memory: the kernel's own data at its bottom, the frame pool above.
- **Page tables in the aarch64 format** (4 KiB granule, 48-bit addresses), built by the kernel as now. The emulator walks them.
- **The clock and interrupts:**
  - the time and a one-shot deadline are imports from the page;
  - the devices' interrupts are bits the page sets and the kernel reads at each turn of its loop;
  - a device's MSI becomes such a bit.
- **Preemption** is a budget of user instructions per slice, as unkernel's. Waiting for an interrupt returns to the page's event loop.
- **What it lacks, and says at boot:** no TPM, no UEFI variables, no watchdog, no second CPU (SMP comes in phase 4).

**4.2 The emulator** (`a64`, a crate of its own, no_std, KRN):
- **What it runs:** EL0 A64 integer instructions, the exclusive monitor, and stage-1 translation with the permission bits, through a TLB in software that the kernel flushes.
- **When it stops:** at `svc`, a fault (with its address and kind, as ESR and FAR would say), an undefined instruction, `wfi` or the end of its budget.
- **Device pages:** a load or store to a page that is no RAM calls the page's device model at once.
- **Its tests:**
  - host tests per instruction class;
  - a differential run against the same test programs executed natively on the aarch64 machines (acc-1, acc-2) and under QEMU's user mode.
- **Later:** a JIT of hot blocks to wasm modules, if phase 1's measurements ask for it.

**4.3 The page** (`web/`, TypeScript, no framework, MIT/Apache-2.0):
- **Threads:** a worker runs the kernel. The page's thread only draws and forwards input.
- **The screen:** an OffscreenCanvas in the worker, so no SharedArrayBuffer and no COOP/COEP headers are needed for one CPU. A static host such as GitHub Pages serves it.
- **Its devices:**
  - the serial console;
  - virtio-input, from keyboard and pointer events;
  - virtio-blk: the image fetched by ranges, the writes kept in OPFS or IndexedDB;
  - later, virtio-net over a WebSocket proxy, and sound through WebAudio.
- **The same in Node:** a CI group, "wasm: boot and shell in Node", keeps it from breaking between releases.

## 5. Phases

| Phase | What | Shown by | Estimate (one track) |
|---|---|---|---|
| 0 | **The spike.** The instructions every aarch64 binary of the image uses, counted. The user instructions from boot to the shell, counted by a QEMU plugin. The emulator running one test program, which calls `log` through its mailbox, in Node. | the counts; the program's line | 1–2 weeks |
| 1 | **The shell in Node and in a tab.** `arch/wasm`; the emulator for all the image's instructions; the console, the clock, the read-only disk. | init starts the services and the shell answers `uptime`, `ps`, `ls`; the boot's time | 4–6 weeks |
| 2 | **The desktop in a tab.** The framebuffer, virtio-input, writes kept in the browser; the page published with a link in the README. | `wm`, the shell's window, `fm`, `view`, `edit` used by hand; the tab's download size | 3–4 weeks |
| 3 | **The network.** virtio-net through a WebSocket proxy that the maintainer runs; `get`, `webcheck`, `improve` against the hub. | a fetch from the tab | 2–3 weeks |
| 4 | **Speed and more.** A JIT, SMP over workers (then COOP/COEP), sound. | as measured | as needed |

- **Phases 0–2, a desktop in a tab that anyone can open from the README:** about 2–3 months of the kernel track's work.
- **The first visible result, the shell in a tab:** about 6–8 weeks.
- These are estimates, not promises. Phase 0 tells how much the emulator's speed and its instruction set cost, and the estimate is revised then.

## 6. What it would not give, stated now

- **No evidence carries over** (MC-12.1, 12.9). A browser is a new configuration. What passes there says nothing of a machine, and what passes on a machine says nothing of the browser.
- **Its TCB is larger:** the browser's engine, the page's JS, and the emulator. A fault in the emulator's permission checks is a fault in isolation. A `docs/profile/wasm.md` will say so when phase 1 lands.
- **No hardware's protections:** no TPM, no measured or secure boot, no IOMMU. keystore runs with a passphrase or without a key (308-KRN-0205). What the tab keeps is readable by its origin and by anyone with the browser's profile. It is a place to try MIND Core, not to hold secrets.
- **Speed:** user code runs on an interpreter. The compositor's blend of a scaled screen (213-KRN-0290) would be slow. The tab's screen is taken at the canvas's size, 1:1.

## 7. Risks and unknowns

1. **The emulator's speed.** unkernel's interpreter reaches 250–300 million instructions a second by its own measure. Phase 0 measures how many our boot and desktop need; if ours is several times slower, the JIT moves before phase 2.
2. **Instructions outside the integer set.** The aarch64 build is soft-float throughout today; the x86 build's voice engines use FP (`targets/x86_64-mind-float.json`). An aarch64 program built with FP later needs either a soft-float build for the tab or FP in the emulator.
3. **The device model in JS.** virtio-pci with MSI-X is the larger part of the page's code. A simpler device of our own is the fallback, at the cost of new drivers.
4. **Rust for wasm64.** `wasm64-unknown-unknown` is a tier-3 target: built with `build-std`, as the voice engines' x86 target already is. Binaryen's lowering must be checked on the kernel's code, and its speed measured.
5. **The page's size.** The aarch64 image's programs, voice and fonts must be fetched by ranges, not all at once.

## 8. What the maintainer decides

- **The way:** B, as proposed, or another.
- **The track:**
  - the kernel's `arch/wasm` and the emulator are KRN's;
  - the page (`web/`) is new code that no track owns: KRN's, or a new track's at the maintainer's word.
- **Its number:** a main task, and its tasks numbered by the track that takes them.
- **Hosting:** where the page is published (GitHub Pages needs nothing more for one CPU), and, for phase 3, a WebSocket proxy to run.
- **Whether a teaser of A** (QEMU in wasm, a few days, slow) is wanted before B's phase 1. The kernel track does not recommend it.
