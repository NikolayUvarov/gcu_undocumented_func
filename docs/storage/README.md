# Storage: content identifiers and the block store

**Version:** 0.4 (2026-10-07) · **Track:** `STO` ([TRACKS.md](../../TRACKS.md)), main tasks [300](../../issues/300-checksummed-block-store.md), [301](../../issues-done/301-objects-as-merkle-dags.done), [302](../../issues-done/302-names-and-current-roots.done), [303](../../issues/303-retention-and-collection.md) · **Roadmap:** track B · **Constitution:** [v1.6](../../constitution/EN/MIND_CORE_Constitution_v1.6.md) Article 4

This document describes the storage format of track B as it is built. Only the parts marked **implemented** exist; the rest is plan (MC-12.3). What the platform guarantees is stated in the profile ([docs/profile](../profile/README.md), row "Article 4"), not here.

## Content identifiers — implemented (300-STO-0001)

An immutable representation is named by a **content identifier** (CID): the [CIDv1](https://github.com/multiformats/cid) of multiformats. `libmind/src/cid.rs` reads and writes it, and `libmind/src/sha256.rs` computes its digest.

| Field | Encoding | Supported | Meaning |
|---|---|---|---|
| version | unsigned varint | `1` | the identifier format's version (MC-4.2) |
| content type | unsigned varint, a multicodec code | `0x55` `raw`, `0x71` `dag-cbor` | what the hashed bytes are: `raw` is opaque data, identified as the exact byte sequence; `dag-cbor` is a node of an object (below) (MC-4.2) |
| hash algorithm | unsigned varint, a multihash code | `0x12` `sha2-256` | which hash made the digest (MC-4.2, 4.13) |
| digest length | unsigned varint | `32` | must equal the algorithm's length |
| digest | bytes | SHA-256 of the exact bytes | |

- **Binary form:** the fields in order, 36 bytes for every supported identifier (`01 55 12 20` and the digest).
- **Text form:** the multibase prefix `b` and the binary form in lowercase RFC 4648 base32 without padding, 59 characters, for example `bafkreihdwdcefgh4dqkjv67uzcmw7ojee6xedzdetojuzjevtenxquvyku` for empty content.
- **Type binding.** The content type is part of the identifier, and the digest covers the content's bytes only. The same bytes under another type get another identifier. Only `raw` and `dag-cbor` are accepted.
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

## Objects — implemented, run on the platform (301)

An object larger than a block is a Merkle-DAG named by one root CID (`libmind/src/dag.rs`, Appendix B.3).

- **Chunks:** the object's bytes cut into chunks of 16 KiB, each a `raw` block.
- **Nodes:** DAG-CBOR blocks `{"v": 1, "size": <bytes under the node>, "links": [<CID>, ...]}`, at most 256 links and 10523 bytes. `"v"` is the schema's version, and another is refused (MC-4.13).
- **The shape follows from the size alone:**
  - an object of at most 16 KiB, the empty one too, is a single `raw` block;
  - a larger one has a root of the least height h with size ≤ 16 KiB × 256^h;
  - a node of height k has ⌈size / (16 KiB × 256^(k−1))⌉ children, all full but the last, each of height k − 1;
  - the children of height 1 are chunks.

  So the same bytes always give the same root, however they are written. One node of height 1 covers 4 MiB, height 2 covers 1 GiB, height 3 covers 256 GiB.
- **One encoding.** A node is accepted only if re-encoding it gives exactly its bytes. Integers must be in their shortest form, keys in DAG-CBOR's order with no others, and arrays of definite length. Each link is tag 42 over the identity multibase prefix and a supported CID (MC-4.2).
- **The reader trusts no store.** `size` and `read_at` check every node and chunk on the way from the root against its CID. They also check it against its place in the shape: the link count, the children's types (chunks under height 1, nodes above) and sizes, and the chunk lengths. A block that does not match is refused as corrupt, and a tree out of shape is refused as such.
- **In the block store:** `put` names the content type. A `dag-cbor` block is stored only if `decode` accepts it, and a record typed as a node that does not decode is corrupt (301-STO-0002).
- **Not provided yet:** retention and garbage collection by reachability (MC-4.5). Names are below.

Evidence: `tests/dag_host.rs`.
- The roots and node bytes equal those of an independent reference: the tree built by its shape rule in Python, with the `dag-cbor` and `multiformats` libraries, for sizes around every boundary of the shape.
- Non-canonical nodes, trees out of shape and forged blocks are refused.

On the platform, the QEMU `store` suite (x86 and aarch64, 300-STO-0003) stores an object of 4 MiB + 1 byte through the running store with `blocks pattern`. It gets the reference's root and reads back equal. A damaged chunk refused on the platform is not tested yet ([300-STO-0005](../../issues/300-STO-0005-corruption-on-the-platform.md)).

## The block store — runs at boot on x86 and aarch64 (300-STO-0002, 0003)

`blockstore` serves [`idl/blockstore.wit`](../../idl/blockstore.wit) 1.0 over a block client: `put` takes a content type (`raw` or `dag-cbor`, a node checked before it is stored) and bytes and returns their CID, `get` takes a CID and returns the bytes checked against it, plus `has` and `stat`. Its logic is `blockstore/src/store.rs`. `init` starts it at boot over a RAM disk of its own, `ramdisk#1` ([300-KRN-0001](../../issues-done/300-KRN-0001-blockstore-at-boot.done)). The shell holds a client with every right (slot 25) and lends it for `REQUEST_BLOCKSTORE`. The `blocks` tool uses it.

**Layout (version 1):**

| Sectors | Content |
|---|---|
| 0 | superblock: `MIND-STO`, layout version (u16), sector size (u16), the SHA-256 of these 16 bytes |
| 1 … | records, each starting on a sector. Name records are described under Names. A block record: `MIND-BLK`, layout version (u16), zero (u16), length (u32), the block's CID (36 bytes), the SHA-256 of these 52 bytes (84 bytes in all), then the block's bytes, padded with zeros to the sector |

**Rules:**
- **Only blank sectors are written.** A put writes its record into the first run of blank sectors that holds it, and returns after the device's flush. Sectors of a failed write are not used again until the next scan. Nothing stored is overwritten (MC-4.8); a collection makes freed records blank again (below).
- **Mount.**
  - Only a wholly blank medium is formatted, and only if it is writable. A file system may leave its first sectors zero, so a blank sector 0 alone is not enough.
  - Anything else is refused and left as it is: another file system, a damaged superblock, another layout version (MC-4.13).
  - A store is scanned whole. Every record with a valid header is read and checked against its CID (and a node against the schema), and only intact blocks enter the index.
  - Non-blank sectors outside every record are counted as damaged. The scan resynchronizes at the next valid header, so a damaged header loses only its own record.
- **Reading.** A get reads the record again and checks it against the CID. A block whose bytes do not match is reported corrupt and never returned. It also leaves the index, so a put of the same bytes stores a new copy elsewhere.
- **Limits.**
  - A block is at most 16 KiB, and the index holds 4096 blocks (the service's static memory).
  - A put that finds no room collects once (below); if there is still none, it is refused with `full`. A store with more blocks than the index holds is not mounted at all.
  - Bytes already held are not written again.
  - **One framing.** Only a record's first sector may start with a record's magic (`MIND-BLK`, `MIND-REF`, `MIND-DEL`). A put whose bytes would start a later sector of their record with one is refused with `invalid`; these are 8 given bytes at offsets 428 + 512k of the block. Otherwise a scan that resumes after a damaged header could take a client's bytes for a record. For a block that would only be harmless, since it is checked against its CID, but for a name it would hand over authority (302-STO-0001). An object holding such bytes at those offsets of a chunk cannot be stored yet.
- **Durability.** A put returns once the device has flushed the record. On the RAM disk that means until the next reset, nothing more.
- **What is not provided:**
  - copies on other media (MC-4.8, independence of copies);

Evidence: `tests/blockstore_host.rs`.
- Put and get, and blocks found again after a remount.
- A flipped byte refused when mounting and when reading.
- A damaged header and a torn write each lose only their own record, and a record is written only over blank sectors.
- Full medium, full index, foreign medium, other layout, read-only medium and a failed write are each refused with their error.
- A random sequence of puts, gets and remounts matches a model.

These are host tests of the logic on a simulated medium.

On the platform, the QEMU `store` suite (x86 and aarch64) checks:
- put and get, of objects and of a file;
- a full medium refused with `full` while what it holds stays readable;
- a restarted instance mounting the same medium with every block verified again.

Damage on the platform is not tested yet: no program can change the store's medium ([300-STO-0005](../../issues/300-STO-0005-corruption-on-the-platform.md)).

## Names — run on the platform (302)

A **name** is a stable entity, its **versions** are the immutable roots it has pointed at, and its **head** is the current one (MC-4.3). The three are kept distinct:
- the root is a CID, immutable;
- the name is 1 to 64 bytes of `A-Z a-z 0-9 . _ / -`;
- the version counts publications from 1.

- **Compare-and-swap.** `publish(name, expected, root)` succeeds only if `expected` is the name's current version (0 for a new name), and returns the new version. Of two publishers that read the same version, the first wins and the second gets `conflict`. Nothing of the refused publication is written, and the loser decides what to do: read the new head, merge, try again. This is the declared protocol for concurrent updates.
- **Only complete roots (MC-4.4).** Before a publication, the store checks with `dag::complete` that every block of the root's object is stored. Every node is read and checked against its CID and the shape, and every chunk is looked up. A missing block is refused with `incomplete` and a tree out of shape with `invalid`. A name therefore never points at data the store has not received.
- **Durability.** Blocks are flushed when they are put, and the name record is flushed before the reply. The durability level is what the device's flush gives; on the RAM disk that lasts until the next reset.
- **Record.** One sector, written only into a blank sector: `MIND-REF`, layout version, name length, version, root CID, the name padded with zeros, and the SHA-256 of these 120 bytes. When mounting, the latest valid version of each name is current; a collection frees the records of earlier versions (303-STO-0001).
- **A damaged record.** If the latest record of a name is damaged, it is counted as damaged (`stat`) and the version before it is current. That loses a confirmed change on a damaged medium, but the loss is reported, not silent. Copies on other media are not provided yet (MC-4.8).
- **Boundary (MC-4.10).** One name per publication. There is no transaction across names, and a reader of two names may see one published and the other not.
- **Rights.** `publish` needs `BADGE_PUBLISH` and `resolve` needs `BADGE_GET`. A publication is logged with the name, version, root and the caller's PID.
- **Not provided yet:** removing a name, several names at once, names as roots of retention (MC-4.5).

Evidence: `tests/blockstore_host.rs` (`a_name_changes_only_from_the_version_expected`, `a_root_is_published_only_with_every_block_stored`, `names_are_found_again_after_a_remount`, `a_damaged_name_record_is_reported_and_the_version_before_stands`, `names_are_checked_and_bounded`) and `tests/dag_host.rs` (`an_object_is_complete_only_with_every_block`).

On the platform, the QEMU `store` suite (x86 and aarch64) checks:
- `conflict` for a stale version;
- `incomplete` for a root never stored, with no name created;
- each publication in the store's log;
- the latest version found again after a restart of the service.

A damaged name record on the platform is not tested yet ([300-STO-0005](../../issues/300-STO-0005-corruption-on-the-platform.md)).

## Retention and collection — implemented, run on the platform (303-STO-0001)

The policy (MC-4.5, Appendix B.3):
- **A name's current version retains** everything its object reaches. Earlier versions retain nothing; their records and the blocks only they reached are collected. Explicit history is plan (303-STO-0003).
- **A lease protects a write in progress.** A block no name retains is kept for 60 s after its last put. A put of bytes already held starts the lease again, and so does a mount, when it is done. A writer publishes the root of what it put within that time, or puts again. Leases live in the service's memory and are measured on the monotonic clock.
- **Everything else is collected:**
  - unretained blocks;
  - other copies of a block, corrupt ones too;
  - name records that are not current.
- **A collection runs** when a put or a publication finds no room, and on `collect` (interface 1.1, `BADGE_PUT`). It cannot free what anything retains, so it needs no right beyond storing.

A collection:
- **Mark.** It walks every name's object (`dag::walk`): each node is read, checked against its CID and the shape, and marked, and each chunk is marked. If an object lacks a block or holds a corrupt node, the collection refuses (`incomplete`, `corrupt`) and frees nothing. A block the object still needs is never freed because another went missing.
- **Sweep.** One pass over the medium frees what is not marked or leased. Each record is freed in a crash-safe order:
  1. a one-sector marker over its header (`MIND-DEL`, the sectors it covers, a SHA-256), flushed;
  2. zeros over its other sectors;
  3. zeros over the marker, flushed.

  A mount that finds a marker finishes the free. A stop in the middle never leaves a false report of damage.
- **Room.** The freed sectors are blank, and later records go into them.

Not provided yet:
- **Pins:** retention with an owner, a term and a quota per owner (MC-4.11, B.3). Until then the only owners are names, and the only limit is the medium.
- **History:** a name's earlier versions, with explicit links (MC-4.5).
- **Removing a name** (MC-4.8).
- **Obligations to consumers.**
- **Leases across a restart:** they start again when the store mounts.

Evidence: `tests/blockstore_host.rs`:
- leases and collection, a put that renews its lease, room reused on a full medium;
- nothing freed while a name's object is incomplete, a stopped collection finished by a mount;
- a random model with collections and remounts;
- every record written only over blank sectors.

On the platform, the QEMU `store` suite (x86 and aarch64) checks:
- right after a mount, a collection frees no block;
- once the leases have ended, a fill of a full medium writes new blocks into the room its puts' collections free, and the unreferenced file is gone;
- the named object stays whole, and the store mounts again with no damage.

## Authority — implemented; refusals not exercised on the platform yet (300-STO-0004)

A client's rights come from the badge `init` mints into its capability (`mind::blockstore`), and the service decides every request by it:

| Badge bit | Allows |
|---|---|
| `BADGE_GET` (1) | `get`, `has`, `resolve`, `stat` |
| `BADGE_PUT` (2) | `put`, `collect`, `stat` |
| `BADGE_PUBLISH` (4) | `publish`, `stat` |

- A client with neither bit may do nothing, and bits this version does not know grant nothing. A refusal is answered `rights` and logged with the caller's PID and badge.
- **A CID grants nothing (MC-4.7).** A hash names a representation. It does not permit reading: a get needs `BADGE_GET`, whoever knows the CID.
- **Storing is not reading (MC-4.11).** A put creates a lease, and a publication makes a name retain an object; both are rights apart from reading. There is no per-client quota on them yet: the medium and the index are the only limits. Pins with an owner, a term and a quota are 303-STO-0002.
- **Deduplication (MC-4.7).** Bytes already held are not written again. A client with `BADGE_PUT` can therefore learn whether some bytes are already stored: `stat` does not change and the put is faster. The store treats all its clients as one confidentiality domain. Clients that must not learn of each other's data need separate stores (or a store without deduplication), and none exists yet.

The rule is host-tested (`rights_come_from_the_badge` in `tests/blockstore_host.rs`). On the platform the only client is the shell's, with every right (badge 7), so no refusal can be provoked there yet. A client with fewer rights is requested from the kernel track ([requests-KRN.md](../../issues/requests-KRN.md)).
