# Storage: content identifiers and the block store

**Version:** 0.15 (2026-10-29): the scrub, `blockstore.wit` 2.3 (309-STO-0025); 0.14 (2026-10-29): memory domains, layout 4, `blockstore.wit` 2.2 (308-STO-0023); 0.13 (2026-10-28): sealed records, layout 3, `blockstore.wit` 2.1 (308-STO-0021); 0.12 (2026-10-28): how far a write reached, `blockstore.wit` 2.0, and the store's partition on the stick (307-STO-0019, 0020); 0.11 (2026-10-27): the next stage planned, 307–315; 0.10 (2026-10-10): a model disk imported on the platform, a 3 GiB object on the host; 0.9 (2026-10-09): speech models in the store · **Track:** `STO` ([TRACKS.md](../../TRACKS.md)), main tasks [300](../../issues-done/300-checksummed-block-store.done), [301](../../issues-done/301-objects-as-merkle-dags.done), [302](../../issues-done/302-names-and-current-roots.done), [303](../../issues-done/303-retention-and-collection.done), [304](../../issues-done/304-several-names-at-once.done), [305](../../issues-done/305-recovery-without-the-store.done), [306](../../issues-done/306-checkpoints-and-rebinding.done) · **Roadmap:** track B · **Constitution:** [v1.6](../../constitution/EN/MIND_CORE_Constitution_v1.6.md) Article 4

This document describes the storage format of track B as it is built. Checkpoints of a component's state are in [checkpoints.md](checkpoints.md). Only the parts marked **implemented** exist; the rest is plan (MC-12.3). What the platform guarantees is stated in the profile ([docs/profile](../profile/README.md), row "Article 4"), not here.

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
- Names, retention and collection are below.

Evidence: `tests/dag_host.rs`.
- The roots and node bytes equal those of an independent reference: the tree built by its shape rule in Python, with the `dag-cbor` and `multiformats` libraries, for sizes around every boundary of the shape.
- Non-canonical nodes, trees out of shape and forged blocks are refused.

On the platform, the QEMU `store` suite (x86 and aarch64, 300-STO-0003) stores an object of 4 MiB + 1 byte through the running store with `blocks pattern`. It gets the reference's root and reads back equal. The `storefaults` suite damages a chunk of a stored object on the store's medium; reading the object then fails as corrupt (300-STO-0005, below).

## The block store — runs at boot on x86 and aarch64 (300-STO-0002, 0003)

`blockstore` serves [`idl/blockstore.wit`](../../idl/blockstore.wit) 2.3 over a block client: `put` takes a content type (`raw` or `dag-cbor`, a node checked before it is stored) and bytes and returns their CID, `get` takes a CID and returns the bytes checked against it, plus `has` and `stat`. Its logic is `blockstore/src/store.rs`. `init` starts it at boot over a RAM disk of its own, `ramdisk#1` ([300-KRN-0001](../../issues-done/300-KRN-0001-blockstore-at-boot.done)). The shell holds a client with every right (slot 25) and lends it for `REQUEST_BLOCKSTORE`. The `blocks` tool uses it.

**Layout (version 4: sealed records, 308-STO-0021, and memory domains, 308-STO-0023):**

| Sectors | Content |
|---|---|
| 0 | superblock: `MIND-STO`, layout version (u16), sector size (u16), the key's source (0: none yet, 1: in the clear), three zeros, the SHA-256 of these 16 bytes; the volume key (32 bytes); a check, 32 zero bytes sealed under it (nonce, ciphertext, tag); the SHA-256 of the 140 bytes before it |
| 1 … | sealed records, each starting on a sector (below, Sealed records). Inside its seal, a block record is `MIND-BLK`, layout version (u16), zero (u16), length (u32), the block's CID (36 bytes), the SHA-256 of these 52 bytes (84 bytes in all), then the block's bytes. Name records are described under Names, commit records under Several names at once, pin records under Retention, key records under Memory domains. Each kind of record is checked by its seal, then by its own SHA-256 and the layout version in it |

A store of layout 1 to 3 is refused and left as it is (MC-4.13), with `device` on the wire; the service logs `NOT MOUNTED: A STORE OF LAYOUT 3; THIS BUILD READS LAYOUT 4 AND CONVERTS NONE`. Nothing converts it (AGENTS.md section 3, before the first release). Layout 3 lacked the domains: a build of it would have read a domain's records as corrupt and freed them. The RAM disk starts blank at every boot; a VirtIO disk or a partition that holds an older layout is written again by hand.

**Rules:**
- **Only blank sectors are written.** A put writes its record into the first run of blank sectors that holds it, and returns after the device's flush. Sectors of a failed write are not used again until the next scan. Nothing stored is overwritten (MC-4.8); a collection makes freed records blank again (below).
- **Mount.**
  - Only a wholly blank medium is formatted, and only if it is writable. A file system may leave its first sectors zero, so a blank sector 0 alone is not enough.
  - Anything else is refused and left as it is: another file system, a damaged superblock, another layout version (MC-4.13), a key that does not open the superblock's check (`key`).
  - A store is scanned whole. Every frame's record is opened, and a block is checked against its CID (and a node against the schema); only intact blocks enter the index. A frame whose seal does not open is counted corrupt.
  - Non-blank sectors outside every frame are counted as damaged. The scan resynchronizes at the next frame, so a damaged frame loses only its own record.
- **Reading.** A get reads the record again, opens it and checks it against the CID. A block that does not open or whose bytes do not match is reported corrupt and never returned. It also leaves the index, so a put of the same bytes stores a new copy elsewhere.
- **Limits.**
  - A block is at most 16 KiB.
  - **The index** is a hash table with open addressing (251-STO-0013), 56 bytes a slot. The service sizes it from the medium at mount: room for a block per 8 sectors and at least 4096 blocks, at most 2^20 slots (`slots_for`). It takes as many slots as its memory quota allows, halving down to the least, and logs `[BLOCKSTORE] INDEX: <slots> SLOTS (<KiB> KiB) FOR <sectors> SECTORS`. A large index takes seven eighths of its slots, a small one all of them; `stat` gives that as `CAPACITY`. A medium of more than 2^32 sectors (2 TiB) is refused with `too-large`, since an entry names a sector in 32 bits.
  - An insert or a lookup does not move other entries; a removal moves back only those of its run (deletion without tombstones). 100 000 blocks go in, are found after a remount, and half of them collected leave the rest found (`tests/blockstore_host.rs`).
  - A put that finds no room collects once (below); if there is still none, it is refused with `full`. A store with more blocks than the index holds is not mounted at all.
  - Bytes already held are not written again.
  - **One framing.** A client's bytes are sealed, so none of them is on the medium in the clear, and a scan that resumes after a damaged frame cannot take them for a record. A frame they might look like does not open without the key. Any bytes can be stored (layout 2 refused some at offsets 428 + 512k of a block).
- **Durability.** A put returns once the device has flushed the record. On the RAM disk that means until the next reset, nothing more.
- **What is not provided:**
  - copies on other media (MC-4.8, independence of copies);

Evidence: `tests/blockstore_host.rs`.
- Put and get, and blocks found again after a remount.
- A flipped byte refused when mounting and when reading.
- A damaged frame and a torn write each lose only their own record, and a record is written only over blank sectors.
- Full medium, full index, foreign medium, layout 2, a wrong key, read-only medium and a failed write are each refused with their error.
- A random sequence of puts, gets and remounts matches a model.

These are host tests of the logic on a simulated medium.

On the platform, the QEMU `store` suite (x86 and aarch64) checks:
- put and get, of objects and of a file;
- a full medium refused with `full` while what it holds stays readable;
- a restarted instance mounting the same medium with every block verified again.

**Damage on the platform (300-STO-0005).** No program can change the store's medium: only `blockstore` holds the RAM disk's client (Appendix B.6). The QEMU `storefaults` suite (x86 and aarch64) therefore injects damage from the host. It reads the guest's RAM (QMP `pmemsave`) and flips one bit of every copy of a record's bytes through QEMU's gdbstub, which writes physical memory with the guest stopped. The RAM disk's sectors are such copies. The records are sealed, so the suite finds the one it wants by opening frames: it reads the volume key from the superblock, where it is in the clear for now, and decrypts each frame's first sector with a ChaCha20 of its own (checked against RFC 8439's vector). The suite checks:
- a flipped byte in a chunk of a named object: its seal no longer opens, the object's read stops at `corrupt`, the store logs the chunk's CID, `stat` counts it, and a collection refuses with `incomplete` while the name's object lacks the chunk; a put of the same bytes stores it again elsewhere, the object reads back whole, and the next collection frees the damaged copy;
- a flipped byte found by a new instance mounting the medium: counted `CORRUPT=1`, the chunk left out of the index (`not-found`), and repaired by a put of the same bytes;
- a damaged frame (its magic): the new instance loses that record alone, counts its 33 sectors damaged, and finds the records after it; a put of the same bytes repairs the object;
- a damaged name record (below);
- another key in the superblock, its digest made whole: the new instance is not mounted, logs `NOT MOUNTED: Key` and answers every request with `key`.

This is damage to the medium's bytes between writes and reads, made by the test, not a failure of a real device. A torn write ends as one of the outcomes injected here (a frame whose seal does not open); a device stopping in the middle of a write is tested on the host only. The outcomes are those of the host tests for the same damage; they say nothing about other media or other kinds of fault (MC-12.1).

## Scrub — run on the platform, on request (309-STO-0025)

A damaged record is otherwise found only when it is read or when the store mounts. The scrub reads the medium from its first record to its end and checks each record as a mount does (MC-4.8):
- each record is opened under its domain's key;
- a block's bytes are matched to its CID, and a node to the schema;
- a name, pin, commit or key record is checked by its digest;
- a forgotten domain's records are not opened, since its key is gone: the next collection frees them;
- a record of a domain with no key record is counted corrupt, as a mount counts it;
- non-blank sectors outside every record are counted damaged.

Nothing is written, and nothing is repaired: one copy cannot be repaired from itself ([309](../../issues/309-copies-scrub-and-repair.md)'s copies will). `stat`'s `corrupt` and `damaged` stay the mount's and the reads' counts; the scrub reports its own.

**When it runs (MC-5.1).**
- **Between requests.** After 20 ms without a request, the store checks the records of the next 256 sectors, ending a step at the end of the record or the 33-sector window it is in, then waits again; a request is always served first. A pass starts a day after the last one ended, the mount counting as one.
- **On request.** `scrub` makes a whole pass at once, from the first record, on behalf of a client with the right to collect. That client waits for it, and so does every other: requests are served one at a time, as during a collection. A pass under way between requests is dropped.
- **Writes during a pass.** A record written where the scrub has not reached yet is checked when it gets there. One written over the scrub's place, inside a blank run, moves the scrub to its end: its writer has just checked it, and its middle would otherwise read as damaged. A collection only makes sectors blank.

**What it says.** Each record found corrupt is logged: `[BLOCKSTORE] SCRUB: BLOCK <cid> AT SECTOR <n> CORRUPT` when the index holds a block there, `SCRUB: RECORD AT SECTOR <n> CORRUPT` otherwise. Each whole pass logs `SCRUBBED <records> RECORDS: <c> CORRUPT, <d> DAMAGED SECTORS`, then `ASKED BY PID <pid>` or `BETWEEN REQUESTS`. `scrubbed` returns the last whole pass's findings, the passes since the store started and how long ago the last ended (`blockstore.wit` 2.3, a minor version: no earlier call changes). `blocks scrub` and `blocks scrubbed` show them.

Evidence:
- `tests/blockstore_host.rs`:
  - a pass in steps of one sector finds what a whole pass finds;
  - a damaged block, name record, key record and sector are found, the block named by its CID, and nothing is written;
  - a forgotten domain's records are not counted corrupt, and an unknown domain's are counted as a mount counts them;
  - a record written around the scrub's place is not read as damaged; without the move this test reads 2 damaged sectors.
- QEMU `storefaults` suite, x86 and aarch64: a byte changed in a chunk on the medium is found by `blocks scrub` before anything reads the chunk, logged with its CID and reported by `blocks scrubbed`.

Not tested on the platform: the scrub between requests, whose first pass starts a day after the mount. Its steps are the host tests' steps; its pacing is not measured.

## Names — run on the platform (302, 303-STO-0003, 0004)

A **name** is a stable entity, its **versions** are the immutable roots it has pointed at, and its **head** is the current one (MC-4.3). The three are kept distinct:
- the root is a CID, immutable;
- the name is 1 to 64 bytes of `A-Z a-z 0-9 . _ / -`;
- the version counts publications from 1.

- **Compare-and-swap.** `publish(name, expected, root)` succeeds only if `expected` is the name's current version (0 for a new name), and returns the new version. Of two publishers that read the same version, the first wins and the second gets `conflict`. Nothing of the refused publication is written, and the loser decides what to do: read the new head, merge, try again. This is the declared protocol for concurrent updates.
- **Only complete roots (MC-4.4).** Before a publication, the store checks with `dag::complete` that every block of the root's object is stored. Every node is read and checked against its CID and the shape, and every chunk is looked up. A missing block is refused with `incomplete` and a tree out of shape with `invalid`. A name therefore never points at data the store has not received.
- **Durability.** Blocks are flushed when they are put, and the name record is flushed before the reply. The durability level is what the device's flush gives; on the RAM disk that lasts until the next reset.
- **Record (layout 3).** Sealed in one sector, written only into a blank sector: `MIND-REF`, layout version (u16), name length (u16), version (u64), the root's CID (36 bytes, zero for a removal), the root of the version before (36 bytes, zero for the first), the name padded with zeros to 64 bytes, the kind (0: the version points at its root, 1: it removes the name), zero, the owner (the publisher's badge, u16), and the SHA-256 of these 160 bytes.
- **History (303-STO-0003, MC-4.5).** A name keeps its newest 4 versions: the current one and up to 3 before it. Each record links the root of the version before it, so the links are explicit on the medium. When mounting, the newest valid version of each name is current and the next valid ones are kept up to 4; a collection frees the records of older versions. `history` lists the kept versions, newest first. Every kept version retains its object (Retention, below).
- **Removing a name (303-STO-0004, MC-4.8).** `unpublish(name, expected)` is a compare-and-swap like a publication, and writes a version without a root. The name then resolves to `not-found` and retains nothing; it keeps only that version, and a collection frees the records before it. Removing a name deletes a reference, not data: its objects go only when a collection finds that nothing else retains them (another name, a pin, a lease). Publishing from the removal's version creates the name again with the next number.
- **A damaged record.** If the latest record of a name does not open, it is counted as corrupt (`stat`; damaged if its frame is gone) and the version before it is current. That loses a confirmed change on a damaged medium, but the loss is reported, not silent. Copies on other media are not provided yet (MC-4.8).
- **Listing (300-STO-0018, `blockstore.wit` 1.4).** `names(prefix, after)` gives up to 16 current names that start with `prefix`, in byte order, each after `after`, with its version and root; a removed name is not listed. A client pages through them by asking again after the last name it got, until a page has fewer than 16. Names change between two calls, so a listing in pages is not one point: `snapshot` is. `blocks names [prefix]` prints every page.
- **Boundary (MC-4.10).** A publication or a removal changes one name. Several names change together only through a commit (below); two separate publications are two changes, and a reader may see one and not the other.
- **Rights.** `publish` and `unpublish` need `BADGE_PUBLISH`; `resolve`, `history` and `names` need `BADGE_GET`. Knowing a name grants nothing: reading its object needs `BADGE_GET` still (MC-4.7). A publication and a removal are logged with the name, version, root and the caller's PID. A name is not owned: any client with `BADGE_PUBLISH` may publish its next version or remove it. The owner recorded with a version is the account its object is charged to (below), not a right over the name.
- **Not provided yet:** a history longer than 4 versions, or chosen per name; names that only their publisher may change.

Evidence: `tests/blockstore_host.rs` (`a_name_changes_only_from_the_version_expected`, `a_root_is_published_only_with_every_block_stored`, `names_are_found_again_after_a_remount`, `a_damaged_name_record_is_reported_and_the_version_before_stands`, `names_are_checked_and_bounded`, `each_version_links_the_one_before`, `a_removed_name_retains_nothing_and_keeps_its_version`, `a_collection_frees_what_no_name_retains_once_its_lease_ends`, `names_are_listed_by_prefix_in_pages`) and `tests/dag_host.rs` (`an_object_is_complete_only_with_every_block`).

On the platform, the QEMU `store` suite (x86 and aarch64) checks:
- `conflict` for a stale version;
- `incomplete` for a root never stored, with no name created;
- each publication and removal in the store's log;
- the latest version found again after a restart of the service;
- the history of three versions, a removal refused from a stale version and accepted from the current one, `not-found` after it, and the removal found again after a restart;
- 18 names listed by prefix in two pages and in byte order, every current name without the removed ones, and a prefix listed by the client that may only read (300-STO-0018).

The `storefaults` suite (x86 and aarch64, 300-STO-0005) damages the record of a name's second version on the medium. A new instance counts it corrupt, with a chunk's corrupt copy left from before (`CORRUPT=2`), and the first version is current.

## Sealed records — run on the platform, the key not yet protected (308-STO-0021)

Every record is sealed with ChaCha20-Poly1305 (RFC 8439; the RustCrypto `chacha20poly1305` crate on the platform). What is in the clear on the medium is the superblock and, for each record, its frame of 44 bytes:

| Bytes | Content |
|---|---|
| 0–7 | `MIND-SLD` |
| 8–9 | layout version (u16, 3) |
| 10–11 | domain (u16): 0, the volume's; a memory domain's number; 0xFFFF, the domains' key records (below) |
| 12–15 | the sectors the record covers (u32) |
| 16–27 | nonce |
| 28–43 | tag |

Then the record and zeros up to the end of its last sector, encrypted. So the frame says how many sectors a record takes, which the layout shows anyway, and not what kind of record it is.
- **The associated data** is the frame's first 16 bytes and the record's first sector number. A record copied to another place does not open there.
- **The nonce** is the first 12 bytes of an HMAC-SHA-256 of the associated data and the sealed bytes, under a key derived from the volume key. A write needs no random source, which a qemu64 CPU and the i7-930 lack. The same nonce comes back only for the same record at the same place, which seals to the same bytes.
- **Keys.** Two keys are derived from the volume key by HMAC-SHA-256: one seals, one makes nonces.
- **The volume key** is made at the first mount of a blank medium, or of a partition `make_usb_image.py` formatted with no key yet. It is random where the processor gives random numbers (RDRAND, RNDR). Elsewhere it comes from the medium's size and the time, and the log says so (`VOLUME KEY MADE WITHOUT RDRAND`).
- **The key is kept in the superblock in the clear.** That is **no confidentiality:** whoever reads the medium reads the key and opens every record. What this stage gives is the format, the cipher on the platform and their cost; [308-STO-0022](../../issues/308-STO-0022-the-volume-key-held-by-keystore.md) moves the key to `keystore`. The service's ready line says `SEALED, KEY IN THE CLEAR`.
- **A wrong key.** The superblock holds a check, 32 zero bytes sealed under the key. A key that does not open it is refused at mount with `key` (`blockstore.wit` 2.1), and nothing is read or written. A store of damaged records is not mistaken for one of another key.
- **Corrupt and damaged.** A record whose frame is whole but whose seal does not open is corrupt. Sectors outside any frame are damaged. A block found corrupt at a read is named by its CID from the index; one found at a mount, only counted.
- **A collection** opens every record to learn what it is, and frees a record that does not open unless the index names its place.
- **Cost.** A put hashes its bytes twice (the CID and the nonce) and seals them; a mount and a collection open every record. Every record takes at least one sector, and a block's 44 bytes of frame can add one.

Evidence: `tests/blockstore_host.rs` over a stand-in cipher (a keystream of splitmix64 seeded by SHA-256, a tag of two keyed sums), since the host tests build without crates:
- `nothing_of_a_block_a_cid_or_a_name_is_in_the_clear`: a block's bytes, its CID, a name and every record's magic are nowhere on the medium after puts, a publication, a pin and a commit;
- `a_changed_or_moved_record_does_not_open`: a byte of each part of a frame or of the sealed bytes, and a whole record copied to another place;
- `a_wrong_key_is_refused_at_mount`: another key or check with the digests made whole, or an unknown source, refused with `key` and nothing written; the key changed without its digest is a damaged superblock;
- `a_partition_holding_only_the_superblock_mounts_and_gets_a_key`, `without_a_random_source_the_key_is_derived_and_said_so`, and `hmac_sha256_matches_rfc_4231`.

The platform's cipher is the crate, tested by its own project. On the platform, the QEMU `store` suite (x86 and aarch64) puts, reads and names objects through sealed records, and finds them again after a restart without making a key again. The `storefaults` suite finds each record it damages by opening frames (above), and shows a wrong key refused.

**Not provided:** confidentiality of the medium (the key is in the clear). What sealing protects and what it does not is in [the threat model](../profile/threat-model.md) (308-STO-0024).

## Memory domains — run on the platform (308-STO-0023)

A memory domain is a set of names, pins and blocks with a key of its own, so that it can be forgotten for sure by destroying that key (MC-4.8). Domain 0 is the volume's own; every client works there unless it comes through a domain's client.

- **A domain's key** is 32 random bytes from the processor (RDRAND, RNDR). Without a random source no domain is made (`no-random`): a key derived from anything kept could not be destroyed. It is kept in a key record sealed under the volume key: `MIND-KEY`, layout, the domain, its state (1 live, 2 forgotten), its owner, zero, the key, a SHA-256. Its frame's domain is 0xFFFF.
- **Its records** are sealed under keys derived from the domain's key; their frames name the domain. A mount scans the medium once: records of a domain whose key record it has not met yet wait (up to 64) and are opened once the scan is done; past 64, it scans once more, every key known. A tombstone met after a domain's records takes them out of the tables again.
- **Its own blocks.** The index is keyed by CID and domain: the same bytes in two domains are two blocks, so one domain's forgetting cannot leave its blocks readable through another. Names and pins are a domain's too. An owner's quota counts its roots in every domain.
- **Through a client of its own** (`blockstore.wit` 2.2):
  - `domain-create` makes one for the caller's owner, its badge;
  - `domain-client` gives a client whose every call works in that domain, with the caller's rights: `blockstore` mints it from its own endpoint, with a badge from `0x100` up (`mind::blockstore::FIRST_DOMAIN_CLIENT`);
  - `domains` lists the owner's;
  - `domain-forget` forgets one.
  - Only a client of the volume makes, lends, lists or forgets a domain, and only its owner's.
  - `blocks -d <domain> <command>` runs any command of the tool in a domain; `blocks domain new`, `blocks domains` and `blocks domain forget <id>` are the rest.
- **Forgetting** a domain:
  1. a tombstone (its key record of state 2, no key), flushed;
  2. its names, pins and blocks leave the store's tables, and its clients are revoked;
  3. its key record is erased as any record is freed, and its key wiped from memory.

  A mount that finds the tombstone and the key record both (a forget that stopped between the two) erases the key record. The domain's records never open again: a mount neither counts them corrupt nor reads them, and the next collection frees them and then the tombstone, after which the number may be given again.
- **A damaged key record** leaves its domain unknown: its records are counted corrupt and kept, never freed, since only a tombstone says a domain was forgotten.
- **What forgetting holds against:** reading the medium through its interface, even with the volume key, once the key record is erased. **What it does not:** copies a client already read; and a flash medium's spare blocks, which may keep the erased sector beneath its interface. A running system that is compromised is outside it, as for sealing.

Evidence: `tests/blockstore_host.rs`:
- `two_domains_open_only_under_their_own_keys`: the same bytes in the volume and two domains are three records; each domain's records are framed as its own and none opens under the volume key; a name in one is in neither other; all found again after a remount;
- `a_forgotten_domain_is_refused_then_collected`: its names, pins and blocks gone at once, another owner refused; on the medium the tombstone alone; a remount counts its records neither corrupt nor damaged; a collection frees them and the tombstone; the other domain whole;
- `a_forget_stopped_before_its_key_record_was_erased_is_finished_by_the_next_mount`;
- `a_domains_records_before_its_key_record_are_found`: 10 records that wait, and 70, past what waits, found by a second scan;
- `a_domain_needs_a_random_key_and_an_owner`, `a_damaged_key_record_keeps_its_domains_records`.

On the platform:
- the QEMU `store` suite on aarch64 (RNDR): two domains, a block and a name of one unseen from the volume and the other, a domain's client refused a domain, a forgotten domain gone at once and its records collected, the other whole;
- the `store` suite on x86's qemu64, without RDRAND: no domain made, `no-random`, and the log says why;
- the `boot` suite's store disk on x86 with RDRAND: a domain forgotten before a reboot is unreadable after it and its records are collected; another domain's object reads back whole.

## Several names at once — run on the platform (304)

`commit` changes up to 8 names in one step (304-STO-0007, `blockstore.wit` 1.3). Each change names the version it expects (0 for a new name) and either a new root or none, which removes the name. The contract MC-4.10 asks for:

- **Atomicity.** All the changes or none.
  - Before anything is written, the store checks every change: the expected versions, that each root's object is complete, that a removed name exists, and the owner's quota for all the new roots together.
  - If any check fails, the commit is refused with that error (`conflict`, `incomplete`, `not-found`, `quota`, `invalid`, `full`) and nothing is written.
  - The changes are written as one sealed record: a header sector (`MIND-TXN`, layout version, the count, and a SHA-256 over the header and every entry), then one sector per name with a name record's fields. An entry has no magic and no digest of its own, so it never counts as a record by itself.
  - A mount applies a commit only if its seal opens, its digest checks and every entry is valid. A damaged or torn commit changes no name, and is counted corrupt (its sectors damaged if its frame is gone).
- **Isolation.** The service handles one request at a time, so a commit's changes appear together, between two requests. `snapshot` reads up to 8 names in one request, which is a consistent point. Reading the same names with separate `resolve` calls may straddle a commit.
- **Durability.** The record is flushed before the reply, which is the same level as a publication: the device's flush, so on the RAM disk until the next reset.
- **History and retention.** Each entry links the root of its name's version before, as a name record does. The versions a commit made are kept and retain their objects like any other. The commit record stays as long as any name keeps one of its versions, and a collection frees it once none does.
- **Rights.** `commit` needs `BADGE_PUBLISH` and `snapshot` needs `BADGE_GET`. Each change is logged (`COMMITTED <name> VERSION <n> ROOT <cid>`, or `REMOVED`, with the caller's PID).
- **Not provided:** a commit across stores, and reads that hold a snapshot across several requests (MVCC). A commit is bounded to 8 names, and every name's own history still keeps 4 versions.

Evidence: `tests/blockstore_host.rs`:
- `a_commit_changes_every_name_or_none`: every refusal writes nothing and changes no name, and a valid commit with a new name, a change and a removal is found whole after a mount;
- `a_commit_past_the_quota_changes_nothing`;
- `a_damaged_or_torn_commit_changes_no_name`: a flipped byte in the frame, the header or either entry, a write that stopped before the last sector, and a damaged frame;
- `a_commit_record_goes_once_no_name_keeps_its_versions`;
- the random model, which commits two names at a time, sometimes from a stale version.

On the platform, the QEMU `store` suite (x86 and aarch64) commits two names, refuses a commit with a stale version and one with a root never stored (the other name unchanged), reads a snapshot of four names, and finds a commit with a removal whole after a restart. The `storefaults` suite damages a commit's header on the medium: a new instance applies neither change and counts the record corrupt.

## Recovery without the main store — run on the platform (305)

Appendix B.4 asks that the bootstrap and recovery set be available without a working main storage service. MC-6.8 asks that boot and recovery dependencies form no unresolvable cycle, and that a failed component have a recovery boundary or a degradation mode.

- **The recovery set is the boot volume.** It holds the bootloader, the kernel, `init`, the drivers, every service's image (`blockstore.elf` too) and the programs. They are read from the FAT boot volume, which programs cannot write. Nothing in the boot path is a client of the store: `init` starts `blockstore` after its RAM disk, no service needs it to start, and the shell lends its client only to a program that asks for it. The device key lives in `keystore`'s memory, not in the store. So the store depends on the boot volume and nothing depends on the store to boot.
- **A crash.** `init` is the store's lifecycle owner. It restarts a killed or failed `blockstore` up to 3 times in 60 s, then quarantines it until an operator starts it. A restarted instance mounts the same medium and verifies every block again. Requests in flight when it ended fail with `ERR_PEER`, and clients retry on their own terms (MC-6.6).
- **A medium it cannot mount** (another file system, a damaged superblock, another layout) is left as it is and never formatted. The service keeps running and answers every request with the reason (`device`, with `stat` too, 305-STO-0008), and its log names it (`NOT MOUNTED: Foreign`). This is the degradation mode: the rest of the system goes on without the store.
- **Not provided:**
  - a tool that repairs or re-creates an unmountable store (on the RAM disk a reset gives a blank medium, and with it every block is lost);
  - a second copy to recover from (MC-4.8);
  - recovery of the store from another store.
- **Later:** when releases are kept in the store for self-update ([351-STO-0006](../../issues/351-STO-0006-releases-pinned-in-the-store.md)), the boot slots stay on the boot volume. The store holds a copy that can rebuild a slot, not the only source a boot needs.

Evidence: the QEMU `storefaults` suite (x86 and aarch64):
- the store is killed: `init` restarts it (one more start in `svc`), and the new instance mounts the medium with its names and damage counts;
- the superblock's digest is damaged and the service restarted: it logs `NOT MOUNTED: Foreign` and answers `stat`, `resolve` and a put with `device`, while `run clock` starts a program from the boot volume.

## Retention and collection — implemented, run on the platform (303)

The policy (MC-4.5, MC-4.11, Appendix B.3):
- **A name retains** everything the objects of its kept versions reach: the current one and up to 3 before it (History, above). A removed name retains nothing.
- **A pin retains** its object until its owner unpins it (303-STO-0002). A pin is a registered obligation: an id, an owner (the badge of the client that pinned it), and the object's root, in a record of its own on the medium. Only the owner can end it (`unpin`; another owner gets `rights`). Its record is freed when it ends, and the object goes when nothing else retains it.
- **A lease protects a write in progress.** A block nothing else retains is kept for 60 s after its last put. A put of bytes already held starts the lease again, and so does a mount, when it is done. A writer publishes or pins the root of what it put within that time, or puts again. Leases live in the service's memory and are measured on the monotonic clock.
- **Everything else is collected:**
  - unretained blocks;
  - other copies of a block, corrupt ones too;
  - name records of versions a name no longer keeps;
  - pin records of ended pins.
- **A collection runs** when a put, a publication, a removal or a pin finds no room, and on `collect` (`BADGE_PUT`). It cannot free what anything retains, so it needs no right beyond storing.

**Accounts and quotas (303-STO-0002, MC-4.11).** What a name's version or a pin retains is charged to an owner: the badge of the client that published or pinned it. The account is the bytes of the distinct roots the owner retains, through the kept versions of names and through its pins, each root counted once however many names and pins hold it. A root's bytes are its object's size; blocks shared between different objects are counted in each. A publication or a pin that would take the account past the owner's quota is refused with `quota`, and nothing is written. `usage` reports the caller's account, quota, names and pins; `pins` lists the first 32 of its pins. The store holds 64 pins in all (the service's static table); a pin past them is refused with `full`.

The quota is fixed: three quarters of the medium for every owner. Nobody grants it yet, so it bounds what one owner can take, not what several can take together.

A collection:
- **Mark.** It walks every retained object (`dag::walk`): each node is read, checked against its CID and the shape, and marked, and each chunk is marked. If an object lacks a block or holds a corrupt node, the collection refuses (`incomplete`, `corrupt`) and frees nothing. A block the object still needs is never freed because another went missing.
- **Sweep.** One pass over the medium frees what is not marked or leased. Each record is freed in a crash-safe order:
  1. a one-sector marker over its frame (`MIND-DEL`, the sectors it covers, a SHA-256), sealed like any record, flushed;
  2. zeros over its other sectors;
  3. zeros over the marker, flushed.

  A mount that finds a marker finishes the free. A stop in the middle never leaves a false report of damage.

  A block leaves the index before its record is freed (175-STO-0012, audit A05). So a sweep cut by a read, write or flush failure leaves the running store with no erased block indexed: a later put of the same bytes writes them again rather than acknowledging what is gone. Evidence: `tests/blockstore_host.rs` fails each read and each write of a collection in turn and puts every block again (host only: nothing makes the RAM disk fail in the middle of a collection on the platform).
- **Room.** The freed sectors are blank, and later records go into them.

**Pin record (layout 3, sealed).** One sector: `MIND-PIN`, layout version (u16), zero, the id (u32), the owner (u16), zero, the root's CID (36 bytes), and the SHA-256 of these 56 bytes. When mounting, every valid pin record is a pin; a pin's id is never given again while its record exists, and ids go on from the highest found.

Not provided yet:
- **Terms:** a pin lasts until its owner ends it; a pin that ends at a time of its own is not provided (B.3 allows a term or a termination condition; this is the termination condition).
- **Quotas granted per owner** by whoever grants the clients, and a bound on what all owners retain together.
- **Owners apart from badges:** clients that share a badge share an account.
- **Obligations to consumers.**
- **Leases across a restart:** they start again when the store mounts.

Evidence: `tests/blockstore_host.rs`:
- leases and collection, a put that renews its lease, room reused on a full medium;
- nothing freed while a retained object is incomplete, a stopped collection finished by a mount;
- history: an earlier version retains its object until it falls out of the newest 4 (`a_collection_frees_what_no_name_retains_once_its_lease_ends`);
- removal: the name retains nothing, its objects go at the next collection, the name comes back from the removal's version (`a_removed_name_retains_nothing_and_keeps_its_version`);
- pins: an object kept until its owner unpins it, another owner refused, pins found again after a mount (`a_pin_retains_its_object_until_its_owner_unpins_it`);
- quotas: a publication and a pin refused past the quota, a root counted once, another owner's account apart (`an_owner_retains_no_more_than_its_quota`);
- a random model with names, history, collections and remounts;
- every record written only over blank sectors.

On the platform, the QEMU `store` suite (x86 and aarch64) checks:
- right after a mount, a collection frees no block;
- once the leases have ended, a fill of a full medium writes new blocks into the room its puts' collections free; a file nothing retains is gone, and a file kept as a name's earlier version stays;
- the history of three versions; a second name of the same object counted once in `usage`; a publication and a pin past the quota refused with `quota`; a pin of an object a name already retains, listed and charged nothing more; a removal and an unpin;
- the store mounts again with no damage, the removal and the other name as they were, no pin.

## Speech models in the store (251-STO-0013, 0014) — run on the platform; a 3 GiB object on the host

`blocks models import [id]` stores the models of a model disk (`models:`, [251](../../issues/251-model-cache-and-model-disk.md)):
- **The manifest.** `blocks` reads `models:MANIFEST.json`, at most 60 000 bytes, and has the parser service read it (`parse::model`, `mind::models` over `mind::json`): it never parses the disk's JSON itself. For each model the service gives the id, its files with their sizes and SHA-256, and the bounds of the model's entry in the text.
- **One object per model**, written as a stream through `dag::Builder`:

  ```
  MIND-MODEL 1
  id <id>
  entry <n>
  <the manifest's entry: n bytes, licence and terms included>
  file <size> <sha256> <path>      one line per file, in the manifest's order
  data
  <the files' bytes, one after another in that order>
  ```

  Neither `blocks` nor the store holds a model whole: a chunk at a time goes from the file to the store, through the SHA-256 that checks it.
- **Checked as it is read.** A file whose size or SHA-256 differs from the manifest stops that model: it is not named, and the blocks written so far are freed by the next collection once their leases end.
- **Named** `models/<id>`, with compare-and-swap on the name's current version. The name retains the object, so a collection keeps every file of the model.

`blocks models get <id> <path> <file>` resolves the name, reads the object's header and copies one file out, checked against its SHA-256 again.

Tests:
- `tests/models_host.rs` (passes): the JSON reader against Python's `json`, and the manifest of `models/manifest.toml` read model by model as `scripts/models.py` writes it.
- The `disks` check of the `vfs` suite (x86 and aarch64, also with an NVMe boot disk; passed in the local gate of 2026-10-10): a model disk imported into a store disk, its files read back with their SHA-256, also after the store restarted; a model whose file differs is not named.
- `tests/blockstore_host.rs`, `a_three_gibibyte_object_fits` (ignored by default, for a local run): an object of 3 GiB whose chunks are all distinct, through `dag::Builder` into a store on a sparse file of 4.5 GiB (an owner may retain three quarters of the medium, MC-4.11), found after a remount and read back in pieces. Run on 2026-10-10, built with `-O`: 197 380 blocks, 6 504 258 sectors written, an index of 225 654 slots (12 340 KiB) sized for them, and a peak resident set of 16 732 KiB for the whole test process, in 104 s.

Not provided:
- reading a model into a memory object a recognizer maps (251-STO-0015);
- a 3 GiB model on the platform: QEMU without KVM reads about 4 MB/s. The service sizes its index from the medium: a 4.5 GiB disk asks for 2^20 slots (56 MiB) and, within the default memory quota of 16 MiB, gets 2^18 (14 MiB, room for 229 376 blocks) or less; a 3 GiB model needs about 197 000. Not run;
- the store's memory quota for a large index (requests-KRN.md).

## Authority (300-STO-0004)

A client's rights come from the badge `init` mints into its capability (`mind::blockstore`), and the service decides every request by it:

| Badge bit | Allows |
|---|---|
| `BADGE_GET` (1) | `get`, `has`, `resolve`, `history` |
| `BADGE_PUT` (2) | `put`, `collect` |
| `BADGE_PUBLISH` (4) | `publish`, `unpublish`, `pin`, `unpin` |
| any of them | `stat`, and `pins` and `usage` of the client's own owner |

- A client with neither bit may do nothing, and bits this version does not know grant nothing. A refusal is answered `rights` and logged with the caller's PID and badge.
- **A CID grants nothing (MC-4.7).** A hash names a representation. It does not permit reading: a get needs `BADGE_GET`, whoever knows the CID.
- **Storing is not reading (MC-4.11).** A put creates a lease, and a publication or a pin makes the store retain an object; these are rights apart from reading. What a publication or a pin retains is charged to the client's badge and bounded by its quota (Retention, above). A lease is not charged: what a put holds for 60 s is bounded only by the medium and the index.
- **Deduplication (MC-4.7).** Bytes already held are not written again. A client with `BADGE_PUT` can therefore learn whether some bytes are already stored: `stat` does not change and the put is faster. The store treats all its clients as one confidentiality domain. Clients that must not learn of each other's data need separate stores (or a store without deduplication), and none exists yet.

The rule is host-tested (`rights_come_from_the_badge` in `tests/blockstore_host.rs`). On the platform the shell holds two clients: one with every right (badge 7), lent for `REQUEST_BLOCKSTORE`, and one badged get alone (badge 1), lent for `REQUEST_BLOCKSTORE_READ` ([300-KRN-0024](../../issues-done/300-KRN-0024-read-only-blockstore-client.done)). The `store` suite runs `blocksro`, a copy of `blocks` that asks only to read. It reads a stored file, and its `put` and `publish` are refused and logged with badge 1. No client may only store, so a refused read is host-tested only ([300-STO-0011](../../issues/300-STO-0011-a-client-that-may-only-store.md)).

## How far a write reached — run on the platform (307-STO-0019)

`blockstore.wit` 2.0 says how far a write reached before its reply (MC-4.4):

| Class | When | What it means |
|---|---|---|
| `volatile` | the store's medium is a RAM disk (`KIND_RAM`) | nothing outlives a reset |
| `flushed` | any other medium | the medium's driver acknowledged a flush after the write. What that reaches depends on the driver and the device: `virtio_blk`, for one, answers a flush at once when the device offers none. That is asked of the drivers track for each driver ([requests-DRV.md](../../issues/requests-DRV.md)) |
| `copied` | two media flushed | [309](../../issues/309-copies-scrub-and-repair.md); not reached yet |

- `publish`, `unpublish`, `commit` and `pin` reply with the class, and `stat` gives the medium's.
- A `put` reports none: until a name or a pin retains its object, a block is retained only by its lease (MC-4.11), and its class is the medium's.
- The store's log says it once it mounts (`DURABILITY=VOLATILE`), and `blocks` prints it after each write.
- A 1.x client is refused by the major version on the wire.

On the platform today:
- the store runs on its RAM disk, `volatile`, unless `init` gives it a VirtIO disk, `flushed` (300-KRN-0025);
- on a real machine it is `volatile` until [307](../../issues/307-the-stores-own-partition.md) gives it a partition.

## The store's partition on the stick — laid out, not yet used (307-STO-0020)

`make_usb_image.py` puts a third partition after `MIND LOG`:
- 128 MiB of MBR type `0x7F`;
- formatted as an empty store: the store's superblock with no key yet (source 0), then blank sectors; the store makes the key at its first mount (308-STO-0021);
- the image check refuses it if its type, place, size or superblock differ.

A partition is the store's by that superblock, never by being blank (307). Until `init` gives `blockstore` a client bounded to it (`KRN` and `DRV`, `requests-KRN.md` and `requests-DRV.md`), nothing uses it.

## The next stage: the store as a Mind's memory — planned, not built (307–315)

Planned on 2026-10-27 from the kernel session's draft. None of it is a guarantee yet (MC-12.3).

| Main task | What | Order |
|---|---|---|
| [307](../../issues/307-the-stores-own-partition.md) | A partition of the store's own on real machines; durability classes `volatile`, `flushed`, `copied` (`blockstore.wit` 2.0) | first: until then nothing survives a reset on a real machine |
| [308](../../issues/308-encryption-at-rest.md) | Sealed records, a key per memory domain, the volume key from the TPM or a passphrase | before any personal data |
| [309](../../issues/309-copies-scrub-and-repair.md) | Copies on a second medium, scrub and repair | the scrub built ([309-STO-0025](../../issues-done/309-STO-0025-scrub.done)); copies and repair after 307 |
| [310](../../issues/310-the-index-on-the-medium.md) | The index on the medium, 64-bit sectors (a layout after 3), longer histories | when sizes ask for it |
| [311](../../issues/311-storefs-a-file-face.md) | `storefs`: a file view of a named root, with snapshots and history | after 307 |
| [312](../../issues/312-the-minds-memory.md) | A journal, assertions and derived indexes, forgetting by domain | after 308 |
| [313](../../issues/313-persistent-actors.md) | Checkpoints of services with queues, an effect journal | after 308 |
| [314](../../issues/314-models-mapped-from-the-store.md) | Models mapped read-only from the store | after 307 |
| [315](../../issues/315-memory-across-machines.md) | Memory across machines | an intention only |

