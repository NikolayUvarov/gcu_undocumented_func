# Evidence — `x86-64/QEMU-0`

Tests are run as described in the [README](../../README.md) ("Runtime checks"). They are tests, not proofs (MC-12.2); each supports the statement only for this profile and the tested configuration (QEMU, 4 CPUs unless stated).

| Statement | Evidence |
|---|---|
| Ring 3 with IOPL 0; kernel memory not readable or writable; code RX, stack NX; privileged instructions fault; syscall pointers validated; a fault terminates only the task | QEMU `isolation` suite (`tests/isolation_app.rs` cases r, w, t, n, c, o, u, g, s, y, e, h, p) |
| A read-only memory mint maps read-only; a revoked lease is unmapped in its holder, also after the holder dropped the lease capability; a detached block leaves the address space | `isolation` suite, cases m, v, l, d |
| Revocation reaches descendants whose parent was dropped; a mapping cannot be re-shared as a new root | `isolation` suite, case `k` |
| Heap holes are reused; block and byte limits include retained memory; only own blocks are shareable; revoke unmaps by node; detach leaves the address space | Host tests `user_heap::tests` in `tests/runtime.rs` |
| The bootloader names a corrupt or truncated kernel ELF and a missing boot file instead of hanging | `boot` suite |
| Program arguments reach `say`; a bare program name runs it in the foreground | `listen` suite |
| A send, call or receive with a deadline fails with `ERR_TIMEOUT` after it and leaves nothing queued | `isolation` suite, case `k` |
| The `rtc` service rejects requests with a wrong version, an unknown method or stray bits in either word (MIND IDL schema check) | `isolation` suite, case `k` |
| The IDL generator lays out fields within two words, rejects unsupported types and oversize interfaces; code for every v0 feature compiles without warnings; generated bindings are current | `tests/idl_test.py` |
| A shared block cannot be detached; a memory object cannot be copied over IPC, mints only read-only children and is reported sealed once its writable capability is gone | `isolation` suite, case `k` |
| Applications cannot use privileged system calls (input, display, ports, IRQ, MMIO, spawn, platform, device enumeration, process control, halt); an application cannot create more endpoints than its quota; a dropped handle stays invalid after its slot is reused; a minted endpoint has only the masked rights and cannot be widened by re-minting; port and memory sub-ranges are validated; revoke removes children and keeps the parent; a keeper cannot receive but mints a receiver | `isolation` suite, case `k` |
| The application limit is loader's task quota | `normal` suite (`TASK LIMIT REACHED` for the ninth application) |
| Memory of exited, killed or faulted tasks is reclaimed; kernel heap returns to its baseline | `normal`, `heap`, `memory`, `services` suites (`heap_used` baseline checks) |
| Monotonic clock increases and has sub-millisecond resolution (calibrated TSC) | `services` suite (`clock` command) |
| Private heaps are zeroed, quota-limited, page tables reclaimed, no stale TLB entries | `heap` suite |
| Out-of-memory during spawn rolls back completely | `memory` suite |
| Independent instances, focus, Ctrl+Z over UART and PS/2, Esc, kill, logs, task limit | `normal` suite |
| Preemption and SIMD state preservation across CPUs | `busy`, `smp` suites |
| Services started by `init` with their capabilities; single instance (`SERVICE ALREADY RUNNING`); IPC call/reply with memory capabilities; a dead server wakes its waiting client with `ERR_PEER`; VFS over ATA; programs loaded from disk by `loader`; a restarted service serves clients granted before its restart | `services` suite |
| A killed service is restarted by `init` and serves clients started before the failure; after 3 restarts in 60 s it is quarantined until `RUN`; killing `init` halts the system | `services` suite |
| A send queued for a server that dies fails with `ERR_PEER` instead of waiting for a new instance | `isolation` suite, case `f` |
| An endpoint badge set by `CAP_MINT` reaches the receiver, is kept by children and cannot be replaced | `isolation` suite, case `i` |
| Five children sending at once: four wait, the fifth gets `ERR_BUSY`; a reply after the caller's timeout fails with `ERR_PEER`; a memory object moves to a child (the sender's handle dies); revoke removes a capability waiting in a blocked send; a child reading a revoked lease faults | `isolation` suite, cases q, j, z, b, x |
| A killed AHCI driver is restarted after its device was stopped and its DMA cleared; reads continue through the same endpoint | `ahci` suite |
| `init` drops the platform privilege before READY; quotas are reported per task | `services` suite |
| Drivers with MMIO and DMA capabilities work (AHCI, xHCI) | `ahci` suite, USB image smoke (`tests/usb_image_smoke.py`) |
| Audio gateway (AC97 DMA, IRQ over IPC) and text to speech | `audio`, `tts` suites |
| ELF images: fresh `.bss`, relocations, malformed images rejected; user page tables | Host tests `tests/runtime.rs` |

## Not covered by any test

- Behaviour under a malicious DMA device or driver (not claimed).
- Timing bounds of any kind.
- That the lease holder in case `x` ran on another CPU at the moment of the revoke (the cross-CPU path is exercised only when the scheduler placed it there).
- A minted port sub-range (applications hold no ports).
- Playback reaching the audio output in the `listen` suite (QEMU's capture backend has no output file); the kernel's validation of `PLATFORM_CAP` arguments.
