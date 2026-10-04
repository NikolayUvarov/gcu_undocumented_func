# Evidence — `x86-64/QEMU-0`

Tests are run as described in the [README](../../README.md) ("Runtime checks"). They are tests, not proofs (MC-12.2); each supports the statement only for this profile and the tested configuration (QEMU, 4 CPUs unless stated).

| Statement | Evidence |
|---|---|
| Ring 3 with IOPL 0; kernel memory not readable or writable; code RX, stack NX; privileged instructions fault; syscall pointers validated; a fault terminates only the task | QEMU `isolation` suite (`tests/isolation_app.rs` cases r, w, t, n, c, o, u, g, s, y, e, h, p) |
| A read-only memory mint maps read-only; a revoked lease is unmapped in its holder; a detached block leaves the address space | `isolation` suite, cases m, v, d |
| A send, call or receive with a deadline fails with `ERR_TIMEOUT` after it and leaves nothing queued | `isolation` suite, case `k` |
| The `rtc` service rejects requests with a wrong version, an unknown method or stray bits (MIND IDL schema check) | `isolation` suite, case `k` |
| The IDL generator lays out fields within two words, rejects unsupported types and oversize interfaces; generated bindings are current | `tests/idl_test.py` |
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
| Drivers with MMIO and DMA capabilities work (AHCI, xHCI) | `ahci` suite, USB image smoke (`tests/usb_image_smoke.py`) |
| Endpoint badges are set once, kept by children, reported by `CAP_INFO` and delivered with each message | `isolation` suite, case `k` |
| The FAT writer keeps FAT12/16/32 volumes consistent: long and Cyrillic names, files across clusters, truncation, moves, removal, growing directories, formatting; a random sequence matches a model | `tests/fat_host.rs` (`fsck.fat -n`, mtools) |
| The read-only check finds what `fsck.fat` finds — a cut chain, a cross link, a lost cluster, a wrong size, a bad short name — and passes clean volumes; any change marks the volume dirty until a flush | `tests/fat_host.rs`; QEMU `disk` suite |
| Log records carry the source `logd` stamped (a line naming another source keeps its real one); the ring numbers records, drops the oldest with a count, clips text on character boundaries and limits each sender | `tests/logd_host.rs`; QEMU `services` suite (`dmesg`, `logger`) |
| `init` lists, stops, starts and restarts services and stops applications on request; it refuses `init` and the shell; a client granted before a restart reaches the new instance | QEMU `services` suite (`svc`, `top`'s k); `tests/monitor_host.rs` |
| Files written through `vfs_server` reach the disk and survive a reboot; the RAM disk does not; boot files and the disk outside `data/` cannot be written by the user, and not at all by applications; `..` is refused | `vfs` suite, `tools` suite (files), `isolation` case `k` |
| Block writes need the write badge and a writable medium; ATA, AHCI and USB write, flush and read back, the raw image holds the sectors and its file system stays consistent | `tests/block_host.rs`; `block` suite (`tests/block_app.rs` as `vfs_server`, host check of the image, `fsck.fat -n`) |
| Audio gateway (AC97 DMA, IRQ over IPC) and text to speech | `audio`, `tts` suites |
| ELF images: fresh `.bss`, relocations, malformed images rejected; user page tables | Host tests `tests/runtime.rs` |

## Not covered by any test

- Behaviour under a malicious DMA device or driver (not claimed).
- Timing bounds of any kind.
- Revocation of a mapping held by a task running on another CPU (the TLB shootdown path of `CAP_REVOKE`).
- The endpoint queue bound (`ERR_BUSY`) and a server's late reply after the caller's timeout (`ERR_PEER`).
