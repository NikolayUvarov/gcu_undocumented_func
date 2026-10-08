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

## Progress (2026-10-08)

- **Done:**
  - [`300-STO-0001`](../issues-done/300-STO-0001-content-identifiers.done): CIDv1 and SHA-256 in `libmind`.
  - [`300-STO-0002`](../issues-done/300-STO-0002-blockstore-service.done): the `blockstore` service, started at boot over `ramdisk#1` by [300-KRN-0001](../issues-done/300-KRN-0001-blockstore-at-boot.done).
  - [`300-STO-0003`](../issues-done/300-STO-0003-blocks-tool-and-store-suite.done): the `blocks` tool and the `store` suite on x86 and aarch64, covering put and get and a full store refused.
  - [`300-STO-0005`](../issues-done/300-STO-0005-corruption-on-the-platform.done): a corrupt chunk, a damaged header and a damaged name record on the platform, by damage injected from the host (`storefaults` suite, x86 and aarch64).
  - The profile's Article 4 row says what is met and what is not.
- **Open:**
  - [`300-STO-0004`](300-STO-0004-rights-by-badge.md): rights by badge. The rule and the service's checks are done; the refusals on the platform wait for a client with fewer rights ([requests-KRN.md](requests-KRN.md)).

The acceptance criteria below are met; the main task stays open until 300-STO-0004 is done.

## Acceptance criteria

- Host tests of the CID format with test vectors.
- A QEMU suite: put and get on the RAM disk, a corrupt block refused, and a full store refused.
- A `docs/profile` entry for Article 4 that says what is met and what is not.

## Related

ROADMAP track B; `ramdisk`, `vfs_server`, `idl/block.wit`; Constitution Article 4.
