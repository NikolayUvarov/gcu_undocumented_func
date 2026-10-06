# Requests for the kernel track (KRN), not numbered yet

**Owner:** kernel track · **Status:** open · **Recorded by:** the storage track (STO), 2026-10-06

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
