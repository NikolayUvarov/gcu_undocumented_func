# 308-STO-0021 — Sealed records: layout 3 with ChaCha20-Poly1305

**Type:** storage (sealing at rest) · **Owner:** `STO` track · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Main task:** [308](308-encryption-at-rest.md) · **Constitution:** MC-4.2, MC-4.8, MC-12.3, MC-12.4

Split from [308](308-encryption-at-rest.md) (2026-10-28).

## Problem

Every record of the store is plain on its medium: blocks, CIDs, names, pins, commits. A medium read in another machine gives all of it away. [308](308-encryption-at-rest.md) seals the records before any personal data is stored. This task is its format and its cipher; where the key lives is [308-STO-0022](308-STO-0022-the-volume-key-held-by-keystore.md).

## Plan

- **Layout 3 replaces layout 2,** which is refused by name, with no conversion code (AGENTS.md section 3).
- **A sealed record:**
  - a plain frame: magic, layout, domain (0 until [308-STO-0023](308-STO-0023-memory-domains.md)), the sectors the record covers, a nonce of 12 bytes and the tag;
  - then the record of layout 2 (block, name, pin, commit or free marker) and the zeros up to the end of its last sector, sealed with ChaCha20-Poly1305 (RFC 8439) under the volume key.
  - Changed while building (2026-10-28):
    - **The nonce is derived, not random:** the first 12 bytes of an HMAC-SHA-256 of the associated data and the sealed bytes, under a key derived from the volume key (a synthetic nonce, as in SIV). The processor's random source is RDRAND or RNDR alone and fails closed (issue 103), and a qemu64 CPU and the i7-930 have neither: random nonces would leave the store unable to write there. A derived nonce repeats only for the same record at the same place, which seals to the same bytes.
    - **The frame gives sectors, not a length,** and the zeros to the sector's end are sealed too: a length would tell a name record (192 bytes) from a pin (88) or a free marker (48).
  - **The associated data** is the frame and the record's first sector number, so a record moved to another place does not open.
  - **Nothing in the clear** but the frame: not the kind of a record, its CID, a name or a root.
- **The superblock** carries the key's source and a check value, so a wrong key is refused at mount by its own reason (`key`, `blockstore.wit` 2.1), not as a store of damaged records.
- **Corrupt and damaged keep their meaning:**
  - a record whose frame is whole but whose seal does not open is corrupt;
  - sectors outside any frame are damaged.

  A block found corrupt at a read is named by its CID from the index; one found at a mount, by its place.
- **The cipher is the RustCrypto `chacha20poly1305` crate** (0.10, already in `tls`), behind a trait in `store.rs`. The host tests drive the store through a stand-in of that trait, and the crate is tested by its own project.
- **The volume key, for now:** it is made at the first mount and kept in the superblock in the clear. That is **no confidentiality:** whoever reads the medium reads the key. The format, the cipher and their cost are exercised until [308-STO-0022](308-STO-0022-the-volume-key-held-by-keystore.md) moves the key out. The profile and docs/storage say so.
- **`make_usb_image.py` writes a superblock with no key yet;** the store makes the key at its first mount.

## Acceptance criteria

- Host tests:
  - a record with one byte changed, and one moved to another place, are refused;
  - the wrong key is refused at mount with `key`;
  - no CID, name or block bytes appear on the medium in the clear;
  - a layout 2 medium is refused by name.
- QEMU, x86 and aarch64: the `store` and `storefaults` suites on sealed records. `storefaults` damages a sealed record from the host and sees it refused.
- docs/storage and the profile's Article 4 row: what is sealed, and that the key is not yet protected.

## Related

[308](308-encryption-at-rest.md), [308-STO-0022](308-STO-0022-the-volume-key-held-by-keystore.md), [308-STO-0023](308-STO-0023-memory-domains.md), [300-STO-0005](../issues-done/300-STO-0005-corruption-on-the-platform.done) (`storefaults`).
