# Platform profile `wasm32/page-0`

**Version:** 0.1 (2026-11-28, 215-PRT-0017) · **Constitution:** [v1.6](../../../constitution/EN/MIND_CORE_Constitution_v1.6.md) · **Main task:** [215](../../../issues/215-mind-core-in-a-browser-tab.md)

MIND Core in a browser tab, or in Node, as [docs/wasm/README.md](../../wasm/README.md) plans it (way B).

**What it shares with [`aarch64/QEMU-virt-0`](../aarch64/README.md):**
- the kernel's generic part;
- every service's and program's binary: the aarch64 build itself, unchanged;
- the system-call interface.

**What differs:**
- the architecture layer, `kernel/src/arch/wasm/`;
- the CPU of the tasks: the `a64` emulator, not hardware;
- the machine: the page, through `web/`.

**It is a place to try MIND Core.** It is not a place to keep secrets or to rely on isolation (below).

## Building and running

```bash
ARCH=aarch64 ./02_build.sh      # the programs: aarch64_root/
./scripts/build_wasm.sh         # web/build/kernel.wasm (wasm64 lowered to wasm32), and web/build/volume.img from the ISO
node web/node/run.mjs web/build/kernel.wasm aarch64_root --volume web/build/volume.img   # the console on this terminal
(cd web && python3 -m http.server 8000)   # then http://localhost:8000/ : the desktop in a tab
```

**Needs:**
- the pinned nightly with `rust-src` (`wasm64-unknown-unknown` is built with `build-std`);
- Node 22 or later, and npm for Binaryen (`web/package.json` pins `binaryen` 132.0.0, a build tool only);
- `mkfs.fat` and `mtools`, for the volume.

## Machine

- **One CPU, the page's thread** (a worker in a tab).
- **Memory:** the module's linear memory, 512 MiB, is the machine's physical memory. The page lays out, above the module's own data:
  - the screen, 1024x768 BGRX;
  - the kernel's heap, 32 MiB;
  - the boot programs;
  - the boot volume;
  - the frame pool.
- **The boot:** as a live boot from an ISO (352-KRN-0268). The volume is the ISO's FAT image, in memory, read-only. `ramdisk#2` serves it to `vfs_server`.
- **The console:** the kernel's console channel (PLATFORM_CONSOLE) on line 1, to and from the page.
- **The clock:**
  - monotonic: the page's `performance.now()`, with a 10 ms tick;
  - wall: the page's `Date.now()` through a PL031 node;
  - CNTVCT counts nanoseconds, and RNDR is the page's `crypto.getRandomValues`.
- **Input:** two virtio-input devices over virtio-pci on bus 0, a tablet and a keyboard, modelled in `arch/wasm/devices.rs` (no MSI-X, INTx lines 3 and 4). The page passes evdev events into them.
- **Not there:**
  - no disk that keeps writes: `ram:` is lost when the tab closes;
  - no network, no sound, no TPM, no UEFI variables, no watchdog, no second CPU.

## Trusted computing base

Everything in `aarch64/QEMU-virt-0`'s TCB that remains: the kernel's generic part and the services each guarantee names. Then, instead of the CPU and firmware:

- **The `a64` emulator** (`a64/`).
  - Its stage-1 walk and permission checks are the isolation between tasks and from the kernel. A fault in it is a fault in isolation.
  - It is checked by host tests per instruction class and the walk (`a64/tests`), and by a differential run against native aarch64 code (215-PRT-0016). Neither is a proof (MC-12.2).
- **`arch/wasm`** and its device model.
- **The engine that runs the module:** the browser's or Node's WebAssembly engine.
  - The module's memory is the machine's whole physical memory.
  - The page's JavaScript can read and write all of it.
- **The page's JavaScript** (`web/`): it lays the memory out and passes every input event.
- **Binaryen's lowering** (wasm64 to wasm32), a build tool whose output runs.

## Threat model, as it differs

- **The page and the browser are trusted with everything.** Whoever controls the tab's origin, an extension, or the browser profile controls the system and reads all its memory.
- **There is no protection of keys.** keystore has no TPM: its device key is made each start and not kept (the boot says so). Nothing typed into the tab should be a secret.
- **There is no hardware isolation between the kernel and the tasks.** The kernel is not mapped into a task's space, and the emulator never runs kernel code. A task reaches only what its page tables map, as far as the emulator's checks are correct.

## Conformance

**No requirement is claimed met on this platform yet** (MC-12.1, 12.9):
- nothing of `aarch64/QEMU-virt-0`'s evidence carries over;
- the runs below show that the system works here. They are not a gate.

## Evidence (runs, not a gate)

All on 2026-11-28, this branch, the aarch64 build of the same tree, in this session's container (Intel Xeon, 2.1 GHz, 4 CPUs).

| What | Where | Result |
|---|---|---|
| Boot to the shell's prompt | Node 22.23, `web/node/run.mjs` with the volume | 3.5, 3.5 and 3.8 s from the start (three runs) |
| `ps`, `uptime`, `ls` at the prompt | the same | 24 tasks listed; `uptime` loaded from the volume and answered; 115 entries listed |
| The desktop | the same, `wm` typed | the menu, the background, the time and date from the PL031 |
| Keys from the page | the same, `--input` | Enter opens wm's run dialog; `fm` and `edit` typed there open in windows |
| In a tab | Chromium (Playwright's, headless), served from localhost | the shell answered 4.3 s after the start; the desktop came up; `fm` opened with the browser's keys; the pointer followed the mouse |

**Speed:** the emulator runs 47 to 51 million instructions a second in Node (215-PRT-0016). wm's moving background takes 8 to 9 s a frame here, so the tab's volume sets `background = none` (`web/wm.conf`).
