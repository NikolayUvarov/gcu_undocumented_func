# Evidence — `x86-64/QEMU-0`

Tests are run as described in the [README](../../README.md) ("Runtime checks"). They are tests, not proofs (MC-12.2); each supports the statement only for this profile and the tested configuration (QEMU, 4 CPUs unless stated).

| Statement | Evidence |
|---|---|
| Ring 3 with IOPL 0; kernel memory not readable or writable; code RX, stack NX; privileged instructions fault; syscall pointers validated; a fault terminates only the task | QEMU `isolation` suite (`tests/isolation_app.rs` cases r, w, t, n, c, o, u, g, s, y, e, h, p) |
| Applications cannot use privileged system calls (input, display, ports, IRQ, MMIO, spawn, platform, device enumeration, process control, halt); an application cannot create more endpoints than its quota; a dropped handle stays invalid after its slot is reused | `isolation` suite, case `k` |
| The application limit is loader's task quota | `normal` suite (`TASK LIMIT REACHED` for the ninth application) |
| Memory of exited, killed or faulted tasks is reclaimed; kernel heap returns to its baseline | `normal`, `heap`, `memory`, `services` suites (`heap_used` baseline checks) |
| Monotonic clock increases and has sub-millisecond resolution (calibrated TSC) | `services` suite (`clock` command) |
| Private heaps are zeroed, quota-limited, page tables reclaimed, no stale TLB entries | `heap` suite |
| Out-of-memory during spawn rolls back completely | `memory` suite |
| Independent instances, focus, Ctrl+Z over UART and PS/2, Esc, kill, logs, task limit | `normal` suite |
| Preemption and SIMD state preservation across CPUs | `busy`, `smp` suites |
| Services started by `init` with their capabilities; single instance (`SERVICE ALREADY RUNNING`); IPC call/reply with memory capabilities; a dead server wakes its waiting client with `ERR_PEER`; VFS over ATA; programs loaded from disk by `loader` | `services` suite |
| Drivers with MMIO and DMA capabilities work (AHCI, xHCI) | `ahci` suite, USB image smoke (`tests/usb_image_smoke.py`) |
| Audio gateway (AC97 DMA, IRQ over IPC) and text to speech | `audio`, `tts` suites |
| ELF images: fresh `.bss`, relocations, malformed images rejected; user page tables | Host tests `tests/runtime.rs` |

## Not covered by any test

- Behaviour under a malicious DMA device or driver (not claimed).
- Timing bounds of any kind.
