# 300 — A checksummed block store with content addresses (track B, first step)

**Type:** main task · **Owner:** `STO` track · **Priority:** P2 · **Status:** in progress · **Blocked by:** — · **Roadmap:** track B "Checksummed block store → CID and immutable blocks" · **Constitution:** MC-4.1, MC-4.2, MC-4.7, MC-4.8, MC-4.13

## Problem

Article 4 (storage) is "not claimed" in the profile: the system reads FAT volumes and has a RAM disk, but no store of immutable, verifiable objects. Track B begins with a block store whose blocks are named by their content and checked on every read. Every later step of track B builds on it: manifests, Head/Refs, retention, checkpoints.

## Plan

The tasks below are planned; the `STO` track numbers them itself and may change the split.

- `300-STO-0001` — the format of a content identifier (CID). It carries the identifier format's version and the hash algorithm's identifier (MC-4.2, 4.13). The hash is SHA-256 from `libmind` if it exists, else one recorded in THIRD_PARTY.md. Host tests with test vectors.
- `300-STO-0002` — a service `blockstore` over a block client, first the RAM disk (`ramdisk`, `idl/block.wit`). `idl/blockstore.wit` 1.0: put (bytes → CID), get (CID → bytes, checked against the CID), has, stat. Nothing is overwritten; a block whose content does not match its CID is reported as corrupt, never returned (MC-4.8).
- `300-STO-0003` — the on-disk layout, a corruption test (a flipped byte is detected) and exhaustion of the medium (a defined refusal).
- `300-STO-0004` — authority: who may put and who may get, by badge (Appendix B.6). Rights to read a block and the obligation to keep it stay separate (MC-4.11): retention is a later task.
- Requests to other tracks:
  - `KRN`: `init` starts the service and grants its block client ([300-KRN-0001](../issues-done/300-KRN-0001-blockstore-at-boot.done), done).
  - `DRV`: a durable block path for after the RAM disk.

## Progress (2026-10-06)

- **Done — `300-STO-0001`** ([record](../issues-done/300-STO-0001-content-identifiers.done)): CIDv1 identifiers (`raw`, SHA-256) and SHA-256 in `libmind` (`cid`, `sha256`); host tests against the FIPS examples and the reference `multiformats` library; [docs/storage](../docs/storage/README.md).
- **In progress — [`300-STO-0004`](300-STO-0004-rights-by-badge.md)**: put and get rights by badge; the rule, the service's checks and a host test are done, the clients' badges are requested from the kernel track.
- **Changed split:** the on-disk layout, the corruption and exhaustion cases of `300-STO-0003` went into `300-STO-0002` as host tests. `300-STO-0003` is now the test tool and the QEMU suite of the acceptance criteria below, and it starts when the service runs.
- **In progress — [`300-STO-0002`](300-STO-0002-blockstore-service.md)**: the `blockstore` service and `idl/blockstore.wit` 1.0 are built and host-tested; starting it at boot with a RAM disk of its own is requested from the kernel track ([300-KRN-0001](../issues-done/300-KRN-0001-blockstore-at-boot.done) (done)), and the QEMU suite waits for it.

## Acceptance criteria

- Host tests of the CID format with test vectors.
- A QEMU suite: put and get on the RAM disk, a corrupt block refused, and a full store refused.
- A `docs/profile` entry for Article 4 that says what is met and what is not.

## Related

ROADMAP track B; `ramdisk`, `vfs_server`, `idl/block.wit`; Constitution Article 4.
