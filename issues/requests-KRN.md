# Requests for the kernel track (KRN), not numbered yet

**Owner:** kernel track · **Status:** open · **Recorded by:** the storage track (STO) and the tools track (APP), 2026-10-06

The kernel track numbers its own tasks (`NNN-KRN-MMMM`), so requests from other tracks wait here. The kernel track turns each into a task and removes it from this file, and the file goes when it is empty.

## Start `blockstore` at boot, with a RAM disk of its own

### Problem

The block store [`300-STO-0002`](300-STO-0002-blockstore-service.md) is built for both architectures (CI step "Block store") and tested on the host (`tests/blockstore_host.rs`), but nothing starts it. Its QEMU suite waits for it to run (issue 300's acceptance criteria).

It needs a block client of its own. The only RAM disk is vfs_server's `ram:` FAT volume. A second client of that disk would not work: `block::serve` (`libmind/src/block.rs`) keeps one transfer buffer for all its clients, so two clients would read into each other's buffer and write each other's sectors.

Each piece below is in the kernel track's files: `common/abi.rs`, `init`, the shell's lending, `loader`'s slot list.

### Plan (a proposal; the kernel track decides)

- **A second RAM disk.** `ramdisk#1` in `SERVICE_INSTANCES`. `init`'s `ramdisk` arm uses the instance's name, as the `virtio_net` arm does.
- **The service at boot:**
  - `blockstore` in `BOOT_SERVICES` after the RAM disks, and `blockstore.elf` in `BOOT_FILES` (`BOOT_IMAGES` + 1).
  - In `init`: a `blockstore` arm with `SLOT_SERVICE` and a client of `ramdisk#1` badged `mind::block::BADGE_WRITE` in slot 2 (`blockstore/src/main.rs` `BLOCK`), and its `HOLDS` line.
  - `02_build.sh` gets `"blockstore:blockstore:blockstore.elf"`. The CI and `ci_local.sh` steps that build it on its own can go then; the STO track removes them if asked.
  - `tests/qemu_smoke.py` `SERVICES` gets `blockstore`.
- **Clients:**
  - A fixed slot `SLOT_BLOCKSTORE` for the shell (`SLOT_DYNAMIC` moves up), lent in `init`'s `shell` arm.
  - `REQUEST_BLOCKSTORE` in `mind::process`. The shell lends the slot for it in `start_with`, and `loader` accepts the slot.
  - Clients are minted with the badges of [300-STO-0004](300-STO-0004-rights-by-badge.md) (`mind::blockstore`): `BADGE_GET` reads, `BADGE_PUT` stores. The service refuses an unbadged client everything. The shell's client gets `BADGE_GET | BADGE_PUT | BADGE_PUBLISH` (302-STO-0001: `BADGE_PUBLISH` makes a name point at a new root). The shell may lend a program a narrower client (`BADGE_GET` only) where that is enough.
- **Task budget:** this adds two services against `MAX_TASKS` = 32 (issue 171).

### Acceptance criteria

- After boot, blockstore's log has `[BLOCKSTORE] READY BLOCKS=0 NAMES=0 SECTORS=1/16384 CORRUPT=0 DAMAGED=0` (8 MiB RAM disk), on x86 and aarch64.
- A program holding `REQUEST_BLOCKSTORE` reaches `idl/blockstore.wit` with the badge it was lent.
- The STO track then adds a test tool and the QEMU suite: put and get, a corrupt block refused, a full store refused.

### Related

[300](300-checksummed-block-store.md), [300-STO-0002](300-STO-0002-blockstore-service.md), [300-STO-0004](300-STO-0004-rights-by-badge.md), [docs/storage](../docs/storage/README.md).

## The rest of issue 171

### Problem

Issue 171 (limits from the hardware) is a kernel main task. Step 1, all RAM on x86-64, is 171-KRN-0001. The tools session did step 2 at the user's request before TRACKS.md gave the kernel files to the kernel track alone: no task or endpoint limit but memory (`171-KRN-0002` in 171's table). Steps 3–6 are left: CPUs from the firmware (`cpu::MAX`), capability spaces that grow, the frame pool's ranges, and a program's memory (the heap window, `APP_MEMORY_MAX`, the block count, the global caps).

### Plan

See [171](171-limits-from-the-hardware.md), "Plan" and "Progress". The monitors' interface lists 8 CPUs (`sysinfo.wit` `cpus`, `sample.busy-low/high`): the tools track changes it once the kernel reports more.

### Acceptance criteria

Those of 171.

## The busy suite's share check on one CPU depends on the host

### Problem

`busy_suite` checks that without a budget the busy loop gets more than 0.6 of its one CPU, measured against the host's wall clock over 3 s. Under TCG a host that deschedules the emulated CPU takes that time from the loop. In a full local run (`--cpu-model max --cpus 1`, nothing else running in the session) it measured 0.52 once and failed; a rerun passed. Eight runs each on `main` (ed08bed) and on the tools branch (909be4f) gave the same spread: 0.62–0.73, mean 0.69. So the margin above 0.6 is small and host noise can cross it.

### Plan

The kernel track decides: measure the loop's share against the guest's own time (the CPU's busy plus idle time from `STAT_CPUS` over the same interval) rather than the host's wall clock, or another way that keeps the check as strict.

### Acceptance criteria

The check keeps its meaning (no budget: the loop takes most of its CPU) and does not fail when the host is busy.
