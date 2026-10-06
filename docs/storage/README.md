# Storage: content identifiers and the block store

**Version:** 0.1 (2026-10-06) · **Track:** `STO` ([TRACKS.md](../../TRACKS.md)), main task [300](../../issues/300-checksummed-block-store.md) · **Roadmap:** track B · **Constitution:** [v1.6](../../constitution/EN/MIND_CORE_Constitution_v1.6.md) Article 4

This document describes the storage format of track B as it is built. Only the parts marked **implemented** exist; the rest is plan (MC-12.3). What the platform guarantees is stated in the profile ([docs/profile](../profile/README.md), row "Article 4"), not here.

## Content identifiers — implemented (300-STO-0001)

An immutable representation is named by a **content identifier** (CID): the [CIDv1](https://github.com/multiformats/cid) of multiformats. `libmind/src/cid.rs` reads and writes it, and `libmind/src/sha256.rs` computes its digest.

| Field | Encoding | Supported | Meaning |
|---|---|---|---|
| version | unsigned varint | `1` | the identifier format's version (MC-4.2) |
| content type | unsigned varint, a multicodec code | `0x55` `raw` | what the hashed bytes are; `raw` is opaque data, identified as the exact byte sequence (MC-4.2) |
| hash algorithm | unsigned varint, a multihash code | `0x12` `sha2-256` | which hash made the digest (MC-4.2, 4.13) |
| digest length | unsigned varint | `32` | must equal the algorithm's length |
| digest | bytes | SHA-256 of the exact bytes | |

- **Binary form:** the fields in order, 36 bytes for every supported identifier (`01 55 12 20` and the digest).
- **Text form:** the multibase prefix `b` and the binary form in lowercase RFC 4648 base32 without padding, 59 characters, for example `bafkreihdwdcefgh4dqkjv67uzcmw7ojee6xedzdetojuzjevtenxquvyku` for empty content.
- **Type binding.** The content type is part of the identifier, and the digest covers the content's bytes only. The same bytes under another type would get another identifier; no type other than `raw` is accepted yet.
- **One encoding per identifier.** A varint written longer than its shortest form, bytes after the digest, another multibase prefix, uppercase text, padding or non-zero bits after the last byte are refused. Two different encodings never name the same identifier.
- **Unknown is refused (MC-4.13).** CIDv0, any other version, content type or hash algorithm, and a digest length other than the algorithm's are errors (`cid::Error`). They are never read as a supported identifier.
- **Ordering.** Identifiers order as their binary forms do, so a sorted index may use either.

Evidence: `tests/cid_host.rs`. SHA-256 is checked against the FIPS 180-2 examples and against Python's `hashlib` at every padding boundary. Identifiers are checked against those computed by the reference library `multiformats` 0.3.1, and every refusal above has a case. These are tests of the code on the host, not a proof (MC-12.2).

### Changing the algorithm or the encoding — plan

A new hash algorithm or content type gets its own code. Identifiers that already exist keep theirs and stay valid. Following MC-4.13, the migration is a protocol of its own, and none exists yet:
- the correspondence between an old and a new identifier is verified by hashing the same content with both algorithms;
- an alias grants no rights;
- roots and retention move under the protocol, not by renaming.

Until it exists, the store accepts only `sha2-256`.

## The block store — implemented, not started yet (300-STO-0002)

`blockstore` serves [`idl/blockstore.wit`](../../idl/blockstore.wit) 1.0 over a block client: `put` takes bytes and returns their CID, `get` takes a CID and returns the bytes checked against it, plus `has` and `stat`. Its logic is `blockstore/src/store.rs`. The service builds for x86-64 and aarch64, but `init` does not start it yet: that is a request to the kernel track ([requests-KRN.md](../../issues/requests-KRN.md)), and the store runs on the host tests' medium only.

**Layout (version 1):**

| Sectors | Content |
|---|---|
| 0 | superblock: `MIND-STO`, layout version (u16), sector size (u16), the SHA-256 of these 16 bytes |
| 1 … | records, each starting on a sector: `MIND-BLK`, layout version (u16), zero (u16), length (u32), the block's CID (36 bytes), the SHA-256 of these 52 bytes (84 bytes in all), then the block's bytes, padded with zeros to the sector |

**Rules:**
- **Append only.** A put writes only sectors after the last non-blank one and returns after the device's flush. Sectors of a failed write are never used again, so nothing stored is overwritten (MC-4.8).
- **Mount.**
  - Only a wholly blank medium is formatted, and only if it is writable. A file system may leave its first sectors zero, so a blank sector 0 alone is not enough.
  - Anything else is refused and left as it is: another file system, a damaged superblock, another layout version (MC-4.13).
  - A store is scanned whole. Every record with a valid header is read and checked against its CID, and only intact blocks enter the index.
  - Non-blank sectors outside every record are counted as damaged. The scan resynchronizes at the next valid header, so a damaged header loses only its own record.
- **Reading.** A get reads the record again and checks it against the CID. A block whose bytes do not match is reported corrupt and never returned. It also leaves the index, so a put of the same bytes stores a new copy after the log.
- **Limits.**
  - A block is at most 16 KiB, and the index holds 4096 blocks (the service's static memory).
  - A put beyond the medium or the index is refused with `full`, and a store with more blocks than the index holds is not mounted at all.
  - Bytes already held are not written again.
- **Durability.** A put returns once the device has flushed the record. On the RAM disk that means until the next reset, nothing more.
- **What is not provided:**
  - deletion, retention and garbage collection (MC-4.5, 4.11);
  - copies on other media (MC-4.8, independence of copies);
  - names and roots (MC-4.3);
  - rights by badge: every client may put and get until 300-STO-0004.

Evidence: `tests/blockstore_host.rs`.
- Put and get, and blocks found again after a remount.
- A flipped byte refused when mounting and when reading.
- A damaged header and a torn write each lose only their own record, and no sector is written twice.
- Full medium, full index, foreign medium, other layout, read-only medium and a failed write are each refused with their error.
- A random sequence of puts, gets and remounts matches a model.

These are host tests of the logic on a simulated medium, not of the service on the platform.

## Authority — plan (300-STO-0004)

Rights to put and to get will be told apart by badge (Appendix B.6). The right to read a block stays separate from the obligation to keep it (MC-4.11); retention is a later task.
