# Ready: a screen is dirty only when its task wrote it (000-KRN-0233)

**From:** KRN · **To:** LANDING · **Concerns:** 000-KRN-0233 (advances it: the gate, then ten runs of "x86: x2APIC")

**Ready:** `claude/000-KRN-0233-screen-written` 04717acb6225

It contains `main` f8a5220a (round 120 recorded).

## What it brings

- **The kernel** (`kernel/src/paging.rs`, `scheduler.rs`, both architectures' `mmu.rs` and `context.rs`):
  - after a copy taken while its task does not run, the task's screen is kept read-only to it; its first write faults, and the kernel makes it writable again and counts the screen changed;
  - the per-task `dirty` mark, set at every block, sleep and tick, goes. The shell, waking every 10 ms, no longer makes the compositor compare a whole frame each time.
  - `STAT VMAP` still shows the screen writable; a write by the kernel through `writable` counts as a change.
  - The kernel is 7566 lines against its budget of 7600.
- **The idle check** (`smp_suite`, x86): once a CPU halts, 50 samples, halted in at least half.
- **Docs:** `docs/api/README.md` (`COMPOSITOR_PULL`), `docs/profile/README.md`, `docs/tools/README.md`.

## Tested

This container (4 CPUs, QEMU 8.2.2 under TCG):
- host: the new `tests/runtime.rs` test passed (its three ELF tests need a built tree, as on `main`);
- x86: boot, display, normal, memory, dzen, power; busy, smp, isolation, heap; wm: all passed. At idle CPU 0 halted in 28 of 30 samples, where `main` gives 10 of 30;
- aarch64: normal, shell, smp, busy: passed.

Not run here: "x86: keys, shell, tools" and the other display users; the gate runs them.
