# Storage: content identifiers and the block store

**Version:** 0.30 (2026-11-04): a store rebuilt from its copy on a second medium, the copy's names listed, `blocks rebuild`, and the copy read beside a store that does not mount, `blockstore.wit` 2.6, host-tested and run in QEMU on x86 and aarch64 (309-STO-0041); 0.29 (2026-11-04): copies of chosen names on a second medium, a damaged block repaired from the other, `blockstore.wit` 2.5, host-tested and run in QEMU on x86 and aarch64; a removal ends the versions before it at a mount too (309-STO-0040); 0.28 (2026-11-04): two roots of a tree compared, `blocks tree diff`, and a name's kept versions read (311-STO-0039); 0.27 (2026-11-04): the recovery key's calls on the wire, `blockstore.wit` 2.4, answered only through a client badged `BADGE_RECOVER`, which no program holds yet (308-STO-0028); 0.26 (2026-11-04): `storefs`'s two writers, a writer cut and its scopes run on the platform through `fsstep` (311-STO-0032, 0037); 0.25 (2026-10-31): assertions about objects, kept in a journal, `blocks assert` (312-STO-0038); 0.24 (2026-10-31): a scope of a store volume, `storefs`'s `scope` and `adopt`, host-tested (311-STO-0037); 0.23 (2026-10-31): a text index derived from a journal, `blocks index` (312-STO-0035); 0.22 (2026-10-30): a journal checked from its head, `blocks journal` (312-STO-0034); 0.21 (2026-10-30): `storefs`'s logic, host-tested, and its service built, not in the images (311-STO-0032); 0.20 (2026-10-30): copies of named roots in a file, `blocks export` and `import`, and repair from them (309-STO-0031); 0.19 (2026-10-30): a working tree published by compare-and-swap, `blocks tree` (311-STO-0029); 0.18 (2026-10-30): the volume key kept by `keystore`, run on the platform (308-STO-0022); 0.17 (2026-10-30): directories as objects, host-tested (311-STO-0027); 0.16 (2026-10-29): the volume key kept by `keystore`, built (308-STO-0022); 0.15 (2026-10-29): the scrub, `blockstore.wit` 2.3 (309-STO-0025); 0.14 (2026-10-29): memory domains, layout 4, `blockstore.wit` 2.2 (308-STO-0023); 0.13 (2026-10-28): sealed records, layout 3, `blockstore.wit` 2.1 (308-STO-0021); 0.12 (2026-10-28): how far a write reached, `blockstore.wit` 2.0, and the store's partition on the stick (307-STO-0019, 0020); 0.11 (2026-10-27): the next stage planned, 307–315; 0.10 (2026-10-10): a model disk imported on the platform, a 3 GiB object on the host; 0.9 (2026-10-09): speech models in the store · **Track:** `STO` ([TRACKS.md](../../TRACKS.md)), main tasks [300](../../issues-done/300-checksummed-block-store.done), [301](../../issues-done/301-objects-as-merkle-dags.done), [302](../../issues-done/302-names-and-current-roots.done), [303](../../issues-done/303-retention-and-collection.done), [304](../../issues-done/304-several-names-at-once.done), [305](../../issues-done/305-recovery-without-the-store.done), [306](../../issues-done/306-checkpoints-and-rebinding.done) · **Roadmap:** track B · **Constitution:** [v1.6](../../constitution/EN/MIND_CORE_Constitution_v1.6.md) Article 4

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
- **In the block store:** `put` names the content type. A `dag-cbor` block is stored only if `decode` accepts it (or `tree::decode` or `journal::decode`, below), and a record typed as a node that does not decode is corrupt (301-STO-0002).
- Names, retention and collection are below.

Evidence: `tests/dag_host.rs`.
- The roots and node bytes equal those of an independent reference: the tree built by its shape rule in Python, with the `dag-cbor` and `multiformats` libraries, for sizes around every boundary of the shape.
- Non-canonical nodes, trees out of shape and forged blocks are refused.

On the platform, the QEMU `store` suite (x86 and aarch64, 300-STO-0003) stores an object of 4 MiB + 1 byte through the running store with `blocks pattern`. It gets the reference's root and reads back equal. The `storefaults` suite damages a chunk of a stored object on the store's medium; reading the object then fails as corrupt (300-STO-0005, below).

## Directories as objects — host-tested (311-STO-0027)

A directory is a set of named files and subdirectories under one root CID (`libmind/src/tree.rs`), the first step of [311](../../issues/311-storefs-a-file-face.md).

- **A directory node** is one DAG-CBOR block `{"v": 1, "entries": [[name, kind, size, link], ...]}`, its keys in DAG-CBOR's order. Each entry is an array of four:
  - `name`: 1 to 255 bytes of UTF-8, without `/` or NUL, neither `.` nor `..`;
  - `kind`: 0 a file, 1 a directory;
  - `size`: a file's bytes, or a directory's number of entries;
  - `link`: the CID, written as an object's node writes its links. A file is `raw` or `dag-cbor` (an object, above); a directory is `dag-cbor`.

  The entries are sorted by the names' bytes, each name once. The empty directory is `{"v": 1, "entries": []}`. `"v"` is the schema's version, and another is refused (MC-4.13).
- **One encoding.** A node is accepted only if re-encoding it gives exactly its bytes, as for an object's nodes.
- **One block at most.** A directory over 16 KiB is refused: 54 entries with names of 255 bytes, about 350 with names of a few. Larger directories, sharded, are not built.
- **The walk** (`tree::walk`) visits every block under a root, a file's or a directory's:
  - a file is walked as an object (`dag::walk`);
  - directories are kept on a stack of at most 32 from the root down; a deeper tree is refused as out of shape. A parent is read again from its CID on the way back, so the walk holds one block;
  - each directory node is checked against its CID and the schema, and each entry against what it names: its kind against the block it links, its size against the file's bytes or the directory's entries;
  - it returns the bytes of the files under the root; a file named twice counts twice. `tree::size` gives the same from the directory nodes and each file's root block alone;
  - **its work is bounded:** a walk is given a number of steps (an entry gone through, a block visited, a file's blocks included) and stops with `too-large` once it has taken them.
- **The files of a tree** (`tree::files`, 351-STO-0006) are visited with their paths below the root, in the names' order, depth first: each file's path, the size its entry gives, and its CID.
  - Each directory is checked against its CID, the schema and its parent's entry, as in the walk.
  - A file is not read: the visit reads it (`dag::read_at`, `dag::each_chunk`), which checks every block against its CID.
  - It is bounded as the walk is: the depth, the path's bytes, and steps (an entry, a directory read).
  - It is for a reader that writes a tree out, such as a slot written again from a release the store keeps.
- **In the block store:**
  - `put` takes a `dag-cbor` block that is an object's node, a directory or a journal's entry (312-STO-0034, below), and refuses anything else (`invalid`); a mount counts a record of none of them as corrupt.
  - The store's walk is the tree's, and a journal's for a journal's entry (below). A publication, a pin or a commit needs every block under its root (`incomplete`), and a tree out of shape is `invalid`.
  - **A walk in the store takes at most four steps for each block its index can hold, and 64 more.** A tree that needs more is refused as out of shape (`invalid`), with nothing written. So a walk's work is bounded by the medium's size (MC-5.1). By count, not run here: a 3 GiB object of distinct chunks takes about 199 000 steps of the about 790 000 its index's 225 654 slots give. A collection marks every block a retained directory reaches, so a directory's files are retained by the name or pin that holds it.
  - An owner is charged the bytes of the files under a directory it retains; a mount reads them again with `tree::size`.
- **Not built yet:** the `storefs` service (a `vfs.wit` provider); its logic is host-tested (below). On the platform, `blocks tree` makes and reads directories through a working tree (next section).

Evidence: host tests of the code, not a proof (MC-12.2).
- `tests/tree_host.rs`:
  - a node's bytes equal an encoding written in the test from the schema, and decode back to the same entries;
  - every byte of a valid node changed to every other value is refused, or reads as exactly what it encodes;
  - names unsorted or twice, bad names, kinds and links, and a node over a block are refused;
  - the walk of a nested tree visits exactly its blocks;
  - a missing block is `not-found`, a changed one `corrupt`; an entry that disagrees with its object, and a tree past the depth, are out of shape;
  - a walk and a sizing take exactly the steps counted, and one step fewer stops them;
  - `tree::files` lists a nested tree's files at their paths, an empty directory none and a file named twice at each path, and each reads back; 300 random trees list as their models do;
  - a missing or changed directory, an entry with the wrong count and a file where the root belongs are refused; a tree past the depth too; a listing takes exactly the steps counted, and a path buffer one byte short stops it;
  - three deliberate changes of `tree::files`, one at a time on a copy outside git, are each caught: no count check, no `/` between names, an entry not counted as a step.
- `tests/blockstore_host.rs`:
  - a published tree keeps its files across a collection and a mount, an unretained one is freed, and a removed one goes;
  - a tree missing a file's block is `incomplete` for a publication, a pin and a commit;
  - a directory node is stored and found again by a mount, and a `dag-cbor` block of neither schema is refused;
  - a tree that takes one step more than the store gives is refused for a publication, a pin and a commit, and one within is published.

## A working tree — host-tested, run in QEMU on x86 and aarch64 (311-STO-0029)

A writer changes a published tree through a **working tree** (`libmind/src/worktree.rs`), the second step of [311](../../issues/311-storefs-a-file-face.md). A published root is never changed in place (MC-4.9): the edits are made in memory and published as a new root.

- **It starts** from the root a name's version gave, and that version; none and 0 are the empty tree of a new name.
- **Edits:** put a file (an object the store holds, with its size: its root is read and the size checked), replace a file with another in one edit (or put it under a name not taken; 311-STO-0032), make a directory, remove a file or an empty directory, move an entry. A path is names separated by `/`, each one a name a directory node takes; a leading `/` is allowed, and the root is the empty path. Each refusal changes nothing:
  - a name a directory refuses, or the root where an edit needs an entry (`Name`);
  - a missing parent (`NotFound`), a file where a directory belongs (`NotDirectory`);
  - a name taken (`Exists`): a put does not replace a file, a replace does, and a replace does not take a directory's place;
  - a directory removed with entries (`NotEmpty`), or moved into itself or under it (`Inside`).
- **What it holds:** each directory an edit changed, with every directory above it, as its node; what no edit touched stays in the store. **Reads** (lookup and list) see the edits and read the rest from the store. A held directory has no CID until it is written out.
- **Bounds.** It holds at most 64 directories (`DIRS`) and 64 KiB of their nodes and names (`BYTES`). An edit past them is refused (`Full`) with nothing changed, until a publication lets them go. A directory over a block, or more than 32 directories down from the root, is refused (`TooLarge`). A move checks the depth of the place it goes to, not the height of what it moves: a tree too deep is refused by the store's walk at the publication (`invalid`), and nothing is published. The tree takes about 85 KiB, so a program keeps it in a static.
- **Write-out** puts the held directories, deepest first, and returns the new root. A subtree no edit touched keeps its CID and is not written again.
- **Publication** is the name's compare-and-swap: `publish` with the version read. If another writer published since:
  - nothing is published, and the writer is told the version it read and the current one: a conflict, never a merge;
  - its working tree stays as it was. What to do next is the writer's, for example to read the new version and make its edits again;
  - the nodes it wrote out are retained by nothing, and a collection frees them once their lease (60 s) has ended.
- **`blocks tree`**, each change one publication:
  - `ls <name> [path]` prints `FILE <bytes> <cid> <name>` or `DIR <entries> <cid> <name>` for each entry, then `<n> ENTRIES IN /<path> OF <name> VERSION <v>`;
  - `cat <name> <path> <file>` copies a file out, every block checked, and prints `READ <n> BYTES SHA-256 <hex>`;
  - `put <name> <file> <path>`, `mkdir <name> <path>`, `rm <name> <path>`, `mv <name> <from> <to>` print `PUBLISHED <name> VERSION <v> ROOT <cid> (<durability>)`, or `CONFLICT <name>: READ AT VERSION <r>, NOW AT <c>; ROOT <cid> NOT PUBLISHED`;
  - a change reads the name's current version, or a version the name keeps, given as the last word (0: the empty tree of a new name). A test plays a second writer so.
- **Not built yet:** sharded directories, and changes to several names at once from working trees (a commit). `storefs`'s logic is the next section.

Evidence: host tests of the code, not a proof (MC-12.2).
- `tests/worktree_host.rs`, through the store's own logic on a simulated medium:
  - each edit is read at once, and each refusal above leaves what the tree holds and every read unchanged;
  - `DIRS`, `BYTES`, a directory over a block and the depth are refused with nothing changed, and the edit refused for a bound is made after a publication;
  - a write-out's root decodes and walks (`tree::walk`) with every block stored; an edit deep in one subtree writes the directories down to it and no others, and every other CID stays;
  - of two writers from one version, the second is told of the conflict with both versions, and nothing of it is published. A collection after the lease frees its nodes, and every version kept stays whole;
  - a medium stopped at each write of a publication, the write not made or only its first sector, mounts with the old root or the new one, each whole, never a mix;
  - 2400 random edits, replaces among them, read and publish as a model of them does;
  - a replace keeps the directory's entries and their order, is refused as the other edits are, and past `DIRS` holds no stored directory.
- The QEMU `store` suite (x86 and aarch64):
  - `blocks tree` makes two directories and three files under a name, one of them two blocks under a node, and lists them;
  - each file reads back, its SHA-256 checked by the suite, and the name's history holds each root;
  - a change read at an older version is refused as a conflict, and the name is unchanged;
  - once the leases have ended, a collection frees the refused tree's root, and every file is still read;
  - a move and two removals are published, and a directory with entries is not removed.

## Two roots compared — host-tested, run in QEMU (311-STO-0039)

A name keeps its last 4 versions, each a whole tree (History, below). **`tree::diff`** (`libmind/src/tree.rs`) says where two of them differ, so a reader sees what a writer changed without reading both trees.

- **What it is given:** two roots, each a directory or none (the empty tree), two buffers of a block, a buffer for the path and a number of steps.
- **What it visits:** each path below the roots where the trees differ, in the order of the names' bytes, depth first:
  - **added:** an entry only in the second tree;
  - **removed:** an entry only in the first;
  - **changed:** an entry in both that names another object, or has another size or kind (a file become a directory among them). Each side's kind, size (a file's bytes, a directory's entries) and CID are given.
- **What it reads:**
  - **an entry the same on both sides** (kind, size and CID) is not read: one CID is one tree (MC-4.2), so the work follows the changes, not the size of the trees;
  - **a directory on one side only** is one change, its entries not listed;
  - **a directory on both sides whose entries differ** is gone into. Each node read is checked against its CID, the schema, and the entry count its parent's entry gives, as `walk` checks them. An entry of one CID with two counts is so read and refused (`shape`);
  - a node missing is `not-found`, a damaged one `corrupt`, a file's node where a directory belongs `shape`.
- **Bounds:**
  - a step is an entry compared or a node read, and past its steps it is `too-large`;
  - more than 32 directories down is `shape`, as for `walk`;
  - a path longer than its buffer is `too-large`. `PATH_MAX`, 32 names of 255 bytes and the `/` between them, holds every path.
- **No heap and no system calls.** Each pair of directories is read again when the comparison comes back up to it, as `walk` reads its own, so it holds two blocks at a time.
- **`blocks tree`:**
  - `diff <name>[@<version>] <name>[@<version>]` prints a line for each change, then `<n> CHANGES FROM <name> VERSION <v> TO <name> VERSION <w>`:
    - `ADDED <kind> <size> <cid> <path>`;
    - `REMOVED <kind> <size> <cid> <path>`;
    - `CHANGED <kind> <size> <cid> TO <kind> <size> <cid> <path>`.

    Its steps are twice the store's bound on a walk, one bound for each tree;
  - `ls` and `cat` read a version the name keeps as `<name>@<version>`, and `@0` is the empty tree. Store names never hold `@`, so the form cannot be mistaken for a name. A version no longer kept is refused: `blocks: tree <name>: no version <v> kept`;
  - two different names can be compared too.
- **What it does not give:**
  - **no merge:** a comparison only reports changes. Nothing applies one tree's changes to another, and a conflict stays a conflict (MC-4.9);
  - **no moves:** a moved entry is reported as removed at one path and added at another, since a tree keeps no identity of a path;
  - **only kept versions:** a version older than the last 4 is gone, with whatever it alone retained.

Evidence: host tests of the code, not a proof (MC-12.2). `tests/tree_host.rs`:
- each kind of change, at the root and two levels down, and against a comparison of the trees' models;
- the trees compared the other way round, each change turned over;
- the empty tree against a tree, and a root against itself, with nothing read;
- 300 pairs of random trees against their models: random edits at random depths, files and directories of the same names;
- a subtree of 200 directories left unread beside one changed file: 10 blocks read, as many as beside a subtree of one;
- refused: a missing node, a changed node, a file or a file's node as a root or as a directory, and an entry whose count disagrees with its node;
- the steps counted to the last one, the depth to 32 directories, the path buffer to its last byte, and a full `PATH_MAX` path;
- a visit's error ends the comparison and is its answer;
- each of 15 changes of the logic made a test fail, among them a resumed pair started again from its first entry, which the tests' bound of a million steps ends.

On the platform, the QEMU `store` suite on x86 and aarch64 (`store_tree_check`), over the versions 5 to 8 that `home` keeps:
- from version 5 to the current one, the move and the removals: `c.bin` removed and `docs/c.bin` added, both naming the CID the suite put, and the directory `docs/deep` removed as one change;
- from version 6 to 7, a file removed two directories down, with its CID;
- from the empty tree (`@0`) to version 5, the root's file and directory added, each as one change;
- the current version against itself, no change;
- version 5 listed and its `c.bin` read back at its old path, and version 6's `docs/deep/b.txt`, removed since, read back, each by SHA-256;
- version 4, no longer kept, refused.

Not run on the platform: a comparison past its steps, its depth or its path buffer, and a node missing or damaged in a comparison (host-tested).

## Files over the store: `storefs` — host-tested, at boot since 311-KRN-0190; two writers, a writer cut and its scopes run in QEMU (311-STO-0032, 0037)

`storefs/src/fs.rs` is the logic of a service that serves `vfs.wit` over the store's names `fs/<volume>`, each a tree of directories (above), the third step of [311](../../issues/311-storefs-a-file-face.md). `storefs/src/main.rs` answers `vfs.wit` 2.6's calls with it, and builds for x86 and aarch64. `init` starts it as a boot image after `blockstore`, with a client of the store, and gives the shell a client badged `mind::fs::BADGE_USER` (311-KRN-0190); `vfs.wit` 2.6's error `conflict` came with 311-KRN-0189.

- **How a files client reaches it** (the kernel track's answer, 2026-10-30): `mind::fs` asks `vfs_server` first, and asks `storefs` for a volume `vfs_server` answers `not-found`, if the program holds a client of it. So `storefs` answers `not-found` for the boot disk's empty name, `ram`, `log` and `models`, whatever their case; any other volume `v` is the store's name `fs/<v>`.
- **A volume** is 1 to 16 bytes of `A-Z a-z 0-9 . _ -`, its case ignored, and none of `vfs_server`'s; another name is `not-found`. Its tree is the name `fs/<volume>`, in lower case.
- **`root`** opens a tree of its own, a working tree (above) at the name's current version, or the empty tree if the name has none. The handles opened from it share that tree.
- **A tree follows its name:** before each call on it, a tree that holds no unpublished data moves to the name's current version if another writer published since. Its open files are read again at their paths, and a handle whose entry is gone, or no longer a file or a directory as it was, is closed, as a removal closes it. A tree that holds unpublished data stays at the version it read until it publishes.
- **Handles** belong to the client that opened them, by its PID and badge: another client's request with one is `invalid`. Whether a root may write is the service's predicate over the client's badge and the volume, or, for a scope's client, its scope (below). A handle never allows more than the one it came from: `write`, `create` and `truncate` need a directory handle that may write, and a file handle may write only if opened with `write`. A path never leaves its handle's directory: `.` and `..` are `invalid`.
- **Reads** go to the store through the working tree, every block checked against its CID. `list` gives at most 16 entries from an index, in the order of the names; an empty page is the end. A file open in the tree is listed, and `stat`ed, with the size written to it.
- **A change of a directory is one publication** by the name's compare-and-swap, made by the call itself: `open-dir` with `create` (every directory it makes), `open` that makes or empties a file, `remove` and `rename`. A removal closes the handles of what it removes, and the handles of what moves follow it.
- **A file's data** is held as the chunks of 16 KiB it changed; the rest is read from the object it was. The handles of one file in one tree share what is written. At the file's `flush` or `close`, through a handle that may write, it is stored as an object, put in the tree in place of the old one in one edit, and published. A directory's `flush` stores every file written below it and publishes them in one version. A file the working tree cannot hold for its bounds is put after the files before it are published, so that flush publishes twice. A flush through a handle that may not write publishes nothing.
- **A conflict:** if another writer published since the tree was read, nothing is published, and the call that would have published is told (`Conflict`). The tree drops what it did not publish and moves to the name's current version, never merging. A file whose written data was dropped so answers the same conflict to every call until it is closed, and its `close` closes it. The writer then makes its change again from the current version. What a refused publication stored is retained by nothing, and a collection frees it once its lease (60 s) has ended.
- **Bounds,** each refused with nothing changed:
  - a file is at most 64 MiB (`FILE_MAX`) after a write or a truncation: `no-space`;
  - 4 trees (`TREES`), 64 handles (`HANDLES`) and 32 open files (`FILES`), and a client (PID and badge) holds at most half of each: 2 trees, 32 handles, 16 files: `handles`;
  - 16 chunks of written files at once (`CHUNKS`, 256 KiB), a client at most 8 (128 KiB), until a flush lets them go; the working tree's `DIRS` and `BYTES`: `no-space`;
  - a handle's path of 512 bytes (`PATH`), a directory over a block, a tree past 32 directories: `no-space`;
  - 16 scopes (`SCOPES`), a client making at most 8: `handles`.

  The state takes 796 KiB, all zeros when new, so a program keeps it in `.bss`.
- **A scope** (311-STO-0037; MC-3.1, 3.4): a client confined to one directory, which the shell can lend a program ([requests-APP.md](../../issues/requests-APP.md)), as `vfs_server`'s `scope` and `adopt` give one on FAT:
  - `scope(dir, writable)` takes a directory handle of the caller's and answers a capability of `storefs`'s own endpoint with a badge of the scope's: the next from `0x100` that no scope holds, wrapping after `0xFFFF`. `storefs` keeps that capability, so the scope's end revokes every copy of it;
  - its client's `root` of the directory's volume, in any case, is that directory, opened at the name's current version as any root; every other root is `denied`. A path never leaves it, as none leaves a handle's directory;
  - it may write only if `writable` and `dir` may: never more than `dir`. A read-only scope of a volume's root is a client that reads the volume and never writes. So a client writes by its badge: `mind::fs::BADGE_USER` every volume, a scope's badge as its scope says, and every other badge none;
  - **its user:** the first task that asks a root with its badge, whatever that root answers, is its only user; another task's root is `denied`. `adopt(scope, task)` makes it a named task's, asked only by the task that made it (by its PID), and is `denied` for a scope another task uses already; a badge of no scope is `not-found`;
  - **its end:** when its user ends, or after a minute (60 s) if nobody took it up. `storefs` is not told of a task's end: it finds both only when it next sweeps its scopes, so a scope ends lazily, at the first of these after its task ended or its minute passed:
    - the next `scope` made, by any client;
    - an `adopt`;
    - a request with a scope's badge, of any scope;
    - a `root`, `open-dir` or `open` while the table of handles, of trees or of open files is full, which also closes the handles of ended clients (`forget`).

    Until then the ended task's scope keeps its capability minted and its handles, its tree and its chunks held, with nothing of them published; no other task can use them, as a scope's first task is its only user. At that sweep its capability is revoked and dropped, `[STOREFS] SCOPE <badge> ENDED` is logged, and its client's handles are closed unpublished, so they answer `invalid`. On the platform (below), the scope of a program that ended or was killed ends when the shell makes the next one, for the next program it starts with a file;
  - **bounds:** 16 scopes (`SCOPES`), 8 made by one client (`SCOPES_EACH`); past them, or if the capability cannot be minted, `handles`, with nothing changed. A scope keeps a path and no tree: a tree is its client's, from its own share, once it opens the root;
  - **its lines:** `[STOREFS] SCOPE 0x100 FOR notes:/docs (WRITABLE)`, or `(READ-ONLY)`, when it is made, and `[STOREFS] SCOPE 0x100 ENDED`.
- **What a scope does not give:** a scope names its directory by a path, not an object, from the volume's root. A directory moved by another writer is not followed, and a new one made at that path is the scope's. A directory removed is `not-found`, and a root open in it is closed as its tree follows the name.
- **Errors** are the logic's own, each with the error of `vfs.wit` 2.6 that says it. A conflict is `conflict`: nothing of the write was stored under the name, and the client reads the file again and writes once more.
- **The service** answers `check`, `format` and `fill` with `denied`: they are `vfs_server`'s. `volume` gives the label `MIND STORE`, no FAT type and no room (the store's own `stat` says it), and a chunk as the cluster. A directory's size is 0, as on FAT, and no time stamps are kept (`modified` is 0). The store's client is in `SLOT_BLOCKSTORE`, where `init` grants it; its first line is `[STOREFS] READY …`, with its bounds.
- **On the platform:**
  - the shell's file commands on a store volume are the `shell` suite's `storefs_check` (the kernel track's), with `edit` writing through the writable scope of its file's directory the shell lends it for `REQUEST_FILE` (311-APP-0121);
  - two writers, a writer cut and the scopes are the `store` suite's `storefs_writers_check`, through `fsstep` (below), on the volume `w:` (the name `fs/w`). The shell lends `fsstep` a writable scope of its file's directory, as it lends one to `edit`;
  - **not run on the platform:** a flush cut at each of its writes to the medium, which the host tests make (below); and `storefs` restarted while a writer holds data. Tried once by hand on x86 (2026-11-04): the writer's next write was `invalid` and nothing of it was published, but the shell's own calls on the volume were `invalid` too, as `mind::fs` keeps the root handles it opened, and the new instance knows none of them; so the suite does not restart it.
- **`fsstep`** (`fsstep/`, a console program in both images, for these checks): it asks for its file (`REQUEST_FILE`), takes the scope the shell lends in `SLOT_FILE` and keeps full paths (`mind::fs::use_scope`), as `edit` does. Its bytes are the pattern `(i*31+7) % 251` at offset `i`, as `blocks pattern`'s. Each line it prints:
  - `fsstep <file> <bytes> <seconds> <bytes>` makes or empties the file, writes the first bytes, prints `FSSTEP HOLDING <n> BYTES` and waits, the bytes held by `storefs` unpublished; then it writes the rest and closes, and prints `FSSTEP CLOSED <total> BYTES`, or the call that failed and its `mind::fs` error: `FSSTEP ROOT: <error>`, `FSSTEP OPEN: <error>`, `FSSTEP WRITE: <error>` (the file then left open, so its data is dropped unpublished when its scope ends) or `FSSTEP CLOSE: <error>` (`FSSTEP CLOSE: Conflict` when another writer published first). These go to the lent client through `vfs.wit` itself, as `mind::fs`'s `File` closes in `drop` and drops close's answer. What it holds is `storefs`'s share of written chunks for a client at most (128 KiB);
  - `fsstep <file> --read-only` makes, from the scope it was lent, a scope of the same directory that does not write (`Dir::scope(false, …)`), and through it prints `FSSTEP SCOPE READ-ONLY OF <directory>`, `FSSTEP ENTRY FILE|DIR <size> <name>` for each entry and `FSSTEP LISTED <n> ENTRIES`, `FSSTEP READ <n> BYTES SHA-256 <hex>` of the file, then `FSSTEP WRITE <file> REFUSED: <error>` for the file opened to write and `FSSTEP CREATE <directory>/fsstep.new REFUSED: <error>` for a new file (`ALLOWED` in place of `REFUSED` if one is not refused; a failed step says `FSSTEP SCOPE:`, `FSSTEP LIST:` or `FSSTEP READ:` and the error);
  - `fsstep <file> --bounds` writes `in its directory` and a line end to the file (`FSSTEP WROTE 17 BYTES TO <file>`), then asks the lent client itself, as `mind::fs` refuses a path outside its scope before asking: `FSSTEP ROOT OF <volume> IS <name>`, the name of the directory its own volume's root is; `FSSTEP OPEN ../fsstep.out REFUSED: <error>` and `FSSTEP OPEN-DIR .. REFUSED: <error>`; `FSSTEP ROOT ram REFUSED: <error>` and `FSSTEP ROOT notes REFUSED: <error>`, `vfs_server`'s RAM disk and another store volume (`ALLOWED` if one is not refused).
- **What the `store` suite's `storefs_writers_check` runs** (x86 and aarch64, the suite's last check):
  - **two writers:** `fsstep w:a/x.bin 20000 15 20000` holds 20000 bytes of the file it made, while the shell writes `w:b.txt` (its creation and its flush, two versions). Its close is told `FSSTEP CLOSE: Conflict`; the name stays at the shell's version, its head in `blocks history fs/w`, and `blocks tree ls` and the shell's `ls` show `x.bin` empty, as its creation left it. Run again without a hold, through a new scope, it publishes, and `blocks tree cat` reads 40000 bytes with the SHA-256 the suite computes from the pattern;
  - **a cut:** `fsstep w:a/y.bin 20000 120 20000` killed (`kill`) while it holds its first 20000 bytes, between its writes and its close: the name stays at the version its creation made and `y.bin` empty; the next scope the shell makes ends the killed task's, its handles closed unpublished, and a write of the file then reads back whole. What is cut is a writer, not a flush: a flush stopped at each of its writes to the medium is host-tested only;
  - **the scopes:** `fsstep w:a/x.bin --read-only` lists both files, reads `x.bin` whole (SHA-256) and is `Denied` a write and a new file, the name unchanged; `fsstep w:a/inside.txt --bounds` writes its file, finds its volume's root to be `a`, and is refused `..` (`Invalid`) and the roots of `ram` and `notes` (`Denied`); `blocks tree ls` finds nothing new outside `a`. `storefs`'s log has each scope made, six writable and the read-only one, in their order, and each but the last one `ENDED`, at the next one's making.

Evidence: host tests of the code, and QEMU runs of the service (above), not a proof (MC-12.2).
- `tests/storefs_host.rs`, through the store's own logic on a simulated medium:
  - `root` (refusing `vfs_server`'s volumes), `open-dir` (directories made on the way), `open` by each mode, `read`, `write` with a gap read as zeros, `truncate` shorter and longer, `stat`, `list` sixteen at a time, `remove`, `rename` across directory handles, `volume`, and their refusals; each change of a directory one version of the name, and a file's data published at its close;
  - a file of three chunks and more, written in pieces, flushed and read back by SHA-256 through its handle, another client's root and the store alone; changed across a chunk's end, and read so through a reader's open file; 400 random writes, truncations and flushes read as a model of them does;
  - the flush of a directory stores the files written below it and publishes them in one version; a directory's changes are published with no flush;
  - a reader's open root, file and directory follow a writer's publications, and its handles of what the writer removed are closed; a tree holding unpublished data stays at its version;
  - of two writers from one version, the second is told of the conflict, `io` on the wire, and the name keeps the first writer's tree and versions; the second tree drops its data and reads the new version, its other handle that held data answers the conflict to every call until closed, and it writes again and publishes;
  - a directory made, a move and a removal each refused on its own call by a conflict, with nothing published, and made once the tree followed the name;
  - a medium stopped at each write of a flush, the write not made or only its first sector, mounts with the old root or the new one, each whole, never a mix;
  - every call with a handle of another PID, or of another badge, is `invalid`; a volume the badge may not write refuses every change; a handle opened to read refuses a write;
  - each bound refused with nothing changed: `FILE_MAX` for a write and a truncation, each table and each client's share of it, the chunks, the path, a path of 32 directories refused at the last leaving none; three files that the working tree cannot hold at once published in two versions;
  - a second client gets its share of trees, handles, files and chunks while the first holds its own;
  - a volume whose root is not a directory is refused;
  - the error mapping, checked against the generated `vfs.wit` 2.6 enum;
  - a scope (311-STO-0037), the clock and whether a task lives given to the logic: its root is its directory, in any case, on its volume alone, another volume `denied` and `..` refused; a read-only scope, one asked writable through a handle that may not write, and a scope made of a handle of either, refuse every change through each handle opened from them, the predicate saying yes; a writable one writes in its directory and nowhere else, the predicate saying no; its first task is its only user, `adopt` by its maker names one and by another task, or of a scope in use, is refused; it ends with its task, or after 60 s and not before if nobody took it up, its handles then `invalid`, what they held unpublished dropped and its tree freed; its directory removed, or moved, is `not-found`, and made again is reached; 16 scopes and 8 a client, past them `handles` with nothing changed, no capability minted and no badge taken; the badges' turn past `0xFFFF` skips one in use. Each of 11 changes of the logic made a test fail: among them a scope writable when its handle is not, another volume allowed, a second user allowed, `adopt` by another task allowed, a scope nobody took up never ended, an ended scope's handles still served, the share not counted, and a badge given while in use.
- `tests/dag_host.rs`: a builder reset between objects makes each one as a new builder does.

## The block store — runs at boot on x86 and aarch64 (300-STO-0002, 0003)

`blockstore` serves [`idl/blockstore.wit`](../../idl/blockstore.wit) 2.5 over a block client: `put` takes a content type (`raw` or `dag-cbor`, a node checked before it is stored) and bytes and returns their CID, `get` takes a CID and returns the bytes checked against it, plus `has` and `stat`. Its logic is `blockstore/src/store.rs`. `init` starts it at boot over a RAM disk of its own, `ramdisk#1` ([300-KRN-0001](../../issues-done/300-KRN-0001-blockstore-at-boot.done)). The shell holds a client with every right (slot 25) and lends it for `REQUEST_BLOCKSTORE`. The `blocks` tool uses it.

**Layout (version 4: sealed records, 308-STO-0021, and memory domains, 308-STO-0023):**

| Sectors | Content |
|---|---|
| 0 | superblock: `MIND-STO`, layout version (u16), sector size (u16), the key's source (0: none yet, 1: in the clear), three zeros, the SHA-256 of these 16 bytes; the volume key (32 bytes); a check, 32 zero bytes sealed under it (nonce, ciphertext, tag); the SHA-256 of the 140 bytes before it |
| 1 … | sealed records, each starting on a sector (below, Sealed records). Inside its seal, a block record is `MIND-BLK`, layout version (u16), zero (u16), length (u32), the block's CID (36 bytes), the SHA-256 of these 52 bytes (84 bytes in all), then the block's bytes. Name records are described under Names, commit records under Several names at once, pin records under Retention, key records under Memory domains, recovery records under The recovery key. Each kind of record is checked by its seal, then by its own SHA-256 and the layout version in it |

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
  - copies on other media (MC-4.8, independence of copies), but of chosen names on a second medium, which `init` lends in QEMU only (Copies on a second medium, below);

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

Nothing is written, and nothing is repaired: one copy cannot be repaired from itself. An import of a copy in a file repairs the blocks it holds (next section). With a copy on a second medium, the scrub reads that medium too and repairs each from the other (Copies on a second medium, below). `stat`'s `corrupt` and `damaged` stay the mount's and the reads' counts; the scrub reports its own.

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

## Copies in a file — host-tested, run in QEMU on x86 and aarch64 (309-STO-0031)

A copy of named roots in a file (`libmind/src/car.rs`, the second step of [309](../../issues-done/309-copies-scrub-and-repair.done)) brings back names a store lost, and repairs blocks a scrub or a read found damaged. It needs no second medium lent to the store: it is made and used by hand, with `blocks`.

- **The format is CAR v1**, the IPLD content archive, so other tools read it:
  - a header: a varint length, then DAG-CBOR `{"roots": [link], "version": 1}`. Its one root is the names node;
  - sections, each a varint length, a CID's binary form and its block: the names node first, then every block under each name's root, in the order the store's walk visits them (`tree::walk`, or a journal's, below). A block met twice is written twice.
- **The names node** is one DAG-CBOR block `{"v": 1, "names": [[name, version, root], ...]}`:
  - 1 to 8 names (one `snapshot` reads 8 at one point), sorted by their bytes, each once, each a name the store takes;
  - `version`: the one the name was at in the store it was exported from, from 1;
  - `root`: a link, written as an object's node writes its links;
  - `"v"` is the schema's version, and another is refused (MC-4.13). The node is the copy's own, not a block the store takes.
- **A copy is checked whole** before anything of it is put:
  - the header and the names node only if re-encoding them gives exactly their bytes. A CAR of another version (CAR v2) is refused;
  - every section's CID a supported one, its length at most a CID and a block (16 KiB) in its shortest form, its bytes matching its CID;
  - each name's root is walked (`tree::walk`, or a journal's walk) over the copy's own sections. Each block the walk visits must be the next section, and each node it reads is checked against its schema and its place. So a copy cut short at any byte, even at a section's end, missing a block, or with one out of the walk's order, is refused, and so is a section after the last;
  - a refusal names its error and the byte its section starts at.
- **`blocks export <file> <name>...`** reads the names' current roots at one point (`snapshot`). It then writes the copy, each block read from the store and checked against its CID, each walk within the steps the store gives its own. It prints `EXPORT <name> VERSION <v> ROOT <cid>` for each name, then `EXPORTED <n> NAMES, <b> BLOCKS, <bytes> BYTES TO <file>; SHA-256 <hex>`.
  - A name that does not exist or is removed, one the store does not take, one given twice, or more than 8 are refused, and nothing is written.
  - The file is made at the first byte written, so a refusal before it leaves a file of that name as it was; a copy that failed part-way is removed.
- **`blocks import --check <file>`** checks the copy and puts nothing. It prints `NAME <name> VERSION <v> ROOT <cid>` for each name, then `CHECKED <file>: <n> NAMES, <b> BLOCKS, <bytes> BYTES; SHA-256 <hex>`, or `blocks: import --check: <file> refused at byte <n>: <error>`.
- **`blocks import <file>`:**
  - checks the copy whole first, and puts nothing if any of it is refused;
  - reads it again and puts each block, after reading it from the store. A block the store holds damaged is found corrupt by that read, leaves the store's index, and the put stores it again: it is repaired. The second read must give the same bytes as the first (`changed`), or nothing is published;
  - reads the names at one point. A name at the copy's root already is left: `SAME <name> VERSION <v> ROOT <cid>`. A name with no root, new or removed, is published at the copy's root by compare-and-swap from the version it is at: `PUBLISHED <name> VERSION <v> ROOT <cid> (<durability>)`. A name at another root is left as it is, never overwritten: `CONFLICT <name>: VERSION <v> ROOT <cid>, NOT <cid>; LEFT AS IT IS`. A name another writer publishes between that read and the publication is read again and reported the same way;
  - prints `IMPORTED <file>: <n> NAMES, <b> BLOCKS (<s> STORED, <r> REPAIRED, <h> HELD), <bytes> BYTES; SHA-256 <hex>`;
  - after a repair, asks for a collection, which frees the damaged copies, so the next scrub does not find them: `COLLECTED <k> BLOCKS: THE DAMAGED COPIES FREED`. A block a mount found damaged has left the index already: the import stores it as one the store lacked, and the next collection frees the damaged copy.
- **A copy is in the clear.** Its blocks and names are not sealed (308-STO-0021): whoever reads the file reads them. It is kept where the user puts it, and is as safe as that place. A memory domain is not exported (`blocks -d <id> export` is refused), as its copy would outlive the domain's forgetting (308-STO-0023).
- **Independence (MC-4.8).** A copy guards against the loss of the store's medium only on another medium:
  - a file on the boot volume is on another medium than the RAM disk store; a file on `ram:` is lost with a reset as the store is;
  - a store on a partition of the stick (307-STO-0020) and a file on the stick's boot volume share one medium: losing the stick loses both;
  - a copy is as old as its export. What was published after it is not in it, and an import does not take a name back to it: a name moved on is a conflict.
- **Not provided:** copies made by the store itself, or on a medium lent to it (309); a name brought back at its old version (an import publishes from the version the name is at here, so into an empty store at 1). The blocks an import puts are retained by their leases (60 s) until its names are published; an import that takes longer is not tested.

Evidence: host tests of the code, not a proof (MC-12.2).
- `tests/car_host.rs`, through the store's own logic on a simulated medium:
  - a copy written by hand from the CAR v1 and DAG-CBOR specifications is what an export writes, byte for byte, and what a check reads;
  - every cut of it and every bit of every byte changed are refused; in a copy of a tree, cuts at and around each section and every 97 bytes, and a changed byte in each section's length, CID and block;
  - a CID of another hash, content type, version or digest length, a section longer than a block, a length that cuts its CID or is not in its shortest form, a header not exactly its encoding, CAR v2, and sections out of order or after the last are refused, each with its error and place;
  - a names node unsorted, with a name twice, not in shortest forms, with its keys out of order or another key, of another schema version, with a name the store does not take, at version 0, or with none or more than 8 names, is refused;
  - each block of a tree left out, or moved one place on, is refused; a directory read again after more nodes than a check remembers is found;
  - export, then import into an empty store, gives the same names at version 1, the same roots and each name's blocks in the same order; imported again, nothing is written; the imported store exports the same copy;
  - a chunk damaged on the medium is found by a scrub; the import repairs it, its collection frees the damaged copy, the next scrub finds nothing, and a new instance mounts the medium whole. A block a mount found damaged is stored again;
  - a name at another root is left and reported, a removed one is published again, one at the copy's root is left; a name another writer published between the read and the publication is a conflict, or the same at the copy's root;
  - a copy with a bad section, wherever it is, or cut short, puts nothing (no write on the medium); one that changed between its check and its import publishes nothing;
  - an export refuses names that do not exist or are removed, are not the store's, are given twice or are more than 8, and a root with a damaged or missing block.
- The QEMU `storefaults` suite (x86 and aarch64): `obj` exported to `ram:` is the CAR v1 the suite writes from the specifications, byte for byte (its SHA-256). A chunk damaged on the medium is found by `blocks scrub` and repaired by `blocks import` of that copy (1 repaired, the damaged copy collected); the next scrub finds nothing, and the object reads back whole.
- The QEMU `store` suite (x86 and aarch64): a tree and a file under two names exported to the boot volume; after a reboot, with the store's RAM disk blank, an import publishes each at its root and every file reads back (its SHA-256 checked by the suite). An import again changes nothing, and a name published since at another root is left as it is.

Not run on aarch64 yet.

## Copies on a second medium — host-tested, run in QEMU on x86 and aarch64 (309-STO-0040)

Chosen names have a copy on a second medium, which the store itself keeps in step, and a block one medium finds damaged is repaired from the other (`blockstore/src/copies.rs`, the third step of [309](../../issues-done/309-copies-scrub-and-repair.done)). `init` lends the second medium in QEMU (309-KRN-0209): the next VirtIO disk after the store's that is blank or the store's, beside a store disk of its own. On real machines it waits for 307's partitions.

- **The copy is a whole store of its own** on the block client `init` lends `blockstore` in slot 4:
  - its own superblock, volume key, index, names, pins and buffers, as described above for the store. A blank medium is formatted as the store's is. Its volume key is random where the processor gives random numbers and kept by `keystore` under its own id (`volume-<id>`) where the secrets client is lent, as the store's;
  - its index is sized by `slots_for` of its medium from what is left of `blockstore`'s memory quota after the store's (64 MiB, 251-KRN-0071), halving down to the least, and logged as `THE COPY'S INDEX: …`;
  - `blockstore` asks for a stack of 256 KiB (`PT_GNU_STACK`, 262-KRN-0164), as `init` and `tally` do: mounting the copy beside the store passes a second store (about 5 KB) and the copies (about 9 KB) through its frames, past the 64 KiB a task has without asking;
  - it is not mounted, and the store runs as it did, with the reason logged (`[BLOCKSTORE] NO COPY: …`): beside a store on a RAM disk, whose writes outlive no reset; on a RAM disk; without memory for its index; or if its own mount fails (another file system, another layout, a key no keeper holds). Without slot 4 nothing is logged. Beside a store that is not mounted it is mounted read-only (309-STO-0041, below), or, a blank medium, not at all (`NO COPY YET: …`).
  - The ready line: `COPY READY BLOCKS=… NAMES=… SECTORS=…/… CORRUPT=… DAMAGED=… SEALED, KEY KEPT BY KEYSTORE`, then `COPY MOUNTED: <n> NAMES WITH A COPY, <b> BEHIND`.
- **Which names have a copy.** A name has one while the copy holds it with a root, so the choice lives where the copy does. Nothing in the store's layout changes: it stays 4.
  - `copy(name, on)` (`blockstore.wit` 2.5) copies the name's current version, the blocks the copy lacks and then the name, and replies `copied`. A name the copy holds already is brought in step.
  - `copy(name, off)` removes the name on the copy (the copy's own `unpublish`), whose next collection frees what only it retained; the reply is the store's own class.
  - **Only the volume's names.** A memory domain's records are sealed under the domain's key, which the copy does not hold: through a domain's client `copy` is refused (`rights`, logged), and a change made in a domain is never followed on the copy.
  - The copy's versions are its own: a name's root is the same on both media, its version numbers may differ. Each copied version is charged on the copy to the owner of the store's version.
- **Each change of a copied name is made on the copy after the store** (`publish`, `unpublish`, `commit`):
  1. the store's change is made as before: it checks the version, the object's completeness and the quota, and is flushed;
  2. every block under the new root the copy lacks is put there: the store's walk visits each block, and one the copy's index does not hold is read from the store, checked against its CID, and put on the copy, whose every put is flushed;
  3. the copy's name is moved to the root by its own compare-and-swap, from the copy's version, or removed there for a removal. A commit's copied names change in one commit on the copy.

  | Reply | When |
  |---|---|
  | `copied` | the name has a copy, and the copy took all three steps; for a commit, every name of it has a copy |
  | `flushed` (`volatile` on a RAM disk) | the name has no copy; or the copy did not take a step: the store's change stands and the name is **behind** (logged `<name> BEHIND ON THE COPY, ITS BLOCKS NOT PUT THERE: Device`, or `ITS NAME NOT MOVED THERE`); or a commit with a name that has none, its copied names still followed |

  A name the store removes loses its copy; published again, it has none until asked.
- **The work follows the change.** Only the blocks the copy lacks are read from the store for it and put there: a second version of a tree that changes one file puts that file's block and the directories above it, and the name. The store's own walk of the root is whole, though: each change of a copied name walks the root twice over the store, its check and the copy's (raw files' roots and every node read, no chunk of a file of several), and once over the copy, the copy's own check of its publication.
- **Behind, and catching up.** The names behind are a list in memory, 64 at most, oldest first; past it, the copy's names are all compared again once the list is empty. Between requests, after 20 ms without one, as the scrub runs (MC-5.1), one name behind is brought in step a step: its current root copied, or its removal followed. One the copy does not take goes to the list's end, and the next catch-up waits 10 s. One name's copy is not divided further.
- **At a mount** each name the copy holds with a root is compared with the store's:
  - the same root: in step;
  - another root, or a removal in the store: behind;
  - **a name the store does not hold at all is left as it is on the copy**, and logged (`<name> ONLY ON THE COPY, VERSION <v> ROOT <cid>: LEFT AS IT IS`): a store that came back blank never empties its copy. Only a removal in the store or `copy(name, off)` removes a name there;
  - if the copy's own mount found records corrupt, every name is behind, so each catch-up walks its root and puts back what the copy lacks, and the copy's next scrub pass ends with a collection.
- **Repair.**
  - **A read** (`get`, in the volume's domain): a block the store finds damaged, or lacks while the copy holds it, is read from the copy, checked against its CID as every read is, put back in the store, and returned, logged `[BLOCKSTORE] REPAIRED <cid> FROM THE COPY`. A block a mount found damaged has left the store's index, so it is told from one never stored only by the copy holding it. One the copy finds damaged too is not returned (`corrupt`).
  - **The scrub** (309-STO-0025) reads the store's medium, then the copy's, a step at a time. For each damaged record whose block the index names, the block is read whole from the other medium, the damaged copy leaves the index, and the block is stored again (`REPAIRED <cid> FROM THE COPY`, `REPAIRED <cid> ON THE COPY FROM THE STORE`); a step repairs 32 at most, and the next pass finds the others. The copy's findings are logged as `COPY SCRUB: …` and `COPY SCRUBBED …`; `scrubbed` and the reply of `scrub` stay the store's pass.
  - **After repairs**, by a read or a scrub, the next pass of that medium ends with a collection there, which frees the damaged records (`COLLECTED <n> BLOCKS ON THE COPY AFTER REPAIRS: THE DAMAGED RECORDS FREED`), so the next scrub finds nothing. One that cannot run (a name lacks a block) is asked for again after the next pass.
  - **A block only one medium holds** is repaired by nothing: it is reported as before, and logged `NOT REPAIRED <cid> ON …: NotFound`.
- **`copy-stat`** (2.5) gives the copy's medium's sectors and those used, its intact blocks, the names it holds a root of, the names behind, and the blocks repaired since the mount, of the store from the copy and of the copy from the store, also while the copy is read beside a store not mounted (309-STO-0041); `no-copy` without a copy, or the store's own error when it is not mounted either. `blocks copy stat` prints `BLOCKS=… NAMES=… SECTORS=<used>/<sectors> BEHIND=… REPAIRED_FROM_COPY=… REPAIRED_FROM_STORE=…`; `blocks copies <name> on|off` prints `COPY ON <name> (COPIED)`, and `publish`, `remove` and `commit` print the class each reply gives, as before.
- **Independence (MC-4.8).** `copy-stat` says nothing about it, and the store cannot tell:
  - two VirtIO disks are two devices to the system, but QEMU may hold both as files on one host disk, which the guest cannot tell;
  - two partitions of one disk are not independent copies ([307](../../issues/307-the-stores-own-partition.md)): losing the disk loses both. On real machines the copy waits for 307's partitions, and the store will say so when it is given one.
- **What it does not give:**
  - a store is rebuilt from its copy name by name, only for the names the store does not hold at all (309-STO-0041, below);
  - **no pairing check:** a medium does not record which store it is the copy of. A copy from another machine's store would have its names reported as only on the copy and left as they are; a name both hold would be moved to this store's root;
  - **pins are not copied:** a pin retains on the medium it was taken on;
  - a block the store's mount found damaged is repaired when it is read; a walk of the store's own that meets it first (a publication, a collection) is refused as before;
  - a block the copy's mount found damaged that only an older version kept there retains is not put back by a catch-up, which walks the current root: the copy's collections are refused until that version falls out of its history;
  - a block a publication's or a collection's own walk finds damaged is not repaired by it: the walk is refused as before, and a read of the block repairs it;
  - copies of more than one medium, and copies on other machines (315).

Evidence: host tests of the logic on simulated media with the stand-in cipher (`tests/blockstore_host.rs`), not a proof (MC-12.2).
- `a_copied_names_blocks_and_name_reach_the_copy_and_its_publication_replies_copied`: a name without a copy replies flushed; on, its node and three chunks then its name reach the copy, at the copy's own version 1, charged to the store's owner; its next publication replies copied and reaches the copy; on again in step writes nothing; both found again after a remount;
- `a_copy_that_does_not_take_a_step_leaves_the_change_flushed_and_the_name_behind_until_caught_up`: the copy's medium refusing writes, or flushes, at the blocks step, and refusing its name record when every block is there already: each change replies flushed, the store's change stands, the copy keeps its version before, the name is behind, still after a catch-up while the medium refuses, in step after one once it takes writes; a removal not taken is followed by the catch-up; on for a name never held, refused, is the copy's error and not behind;
- `a_commits_copied_names_change_in_one_commit_on_the_copy`: one commit record on the copy for two copied names, one moved and one removed, replying copied; a commit with a name without a copy replies flushed, its copied name followed; a commit the copy refuses leaves both names behind, caught up later;
- `a_block_the_store_finds_damaged_is_read_from_the_copy_and_put_back`: a chunk damaged after the mount, read whole from the copy, put back elsewhere and read directly after; one damaged on both media not returned; one damaged before the mount, out of the store's index, read from the copy; the copy's own damage before its mount: the name behind, its block put back by the catch-up; a whole scrub then reports the damaged records, collects them on both media, and the next is clean; in a memory domain the copy is not read;
- `the_scrub_repairs_a_block_damaged_on_either_medium_from_the_other_and_the_next_scrub_is_clean`: in steps of 16 sectors, the store's pass then the copy's, a chunk damaged on each repaired from the other, each pass ending with a collection; the next whole scrub, and each medium's next mount, find nothing;
- `a_block_only_one_medium_holds_is_reported_and_not_repaired`: a name without a copy and a block on the copy alone, each damaged: reported with its CID, not repaired, nothing collected, found again by the next scrub, and a read refused;
- `a_mount_compares_the_copys_names_and_leaves_one_only_the_copy_holds`: another root and a removal in the store behind in the order of their bytes, the same root in step; caught up; with the store's medium blank, the copy's names left as they are and logged, nothing written there, off removing one by the copy's own version, and a name both hold moved to the new store's root;
- `a_removal_in_the_store_is_followed_on_the_copy_and_leaves_the_name_without_one`, `a_memory_domains_name_is_refused_and_its_changes_stay_off_the_copy` (a domain's `copy` invalid, its publication, commit and removal not followed though the volume's name of the same bytes has a copy; catch-up and scrub leave the store in the request's domain);
- `the_blocks_read_and_put_for_a_copy_follow_the_change`: a tree of three files and a subdirectory with a file of a node and three chunks: on puts its 9 blocks and the name (10 writes on the copy); one file changed puts 2 blocks and the name (3 writes), and the store reads its own walk twice and the 2 blocks once more; a file renamed puts 2;
- `random_changes_copies_damage_and_remounts_match_a_model_of_both_media`: 16 rounds of 100 steps over two stores: objects of a block or a directory of two, publications, removals, commits of two names, copy on and off, catch-ups, a block of a name in step damaged on either medium and repaired by a read or a scrub, two scrubs whose second finds nothing, and the copy's medium refusing writes a quarter of the steps; between rounds both remounted, sometimes after damage on either medium. After each step, what each medium holds is checked against the model: every name and its root on the store and on the copy, the names behind, every block under each root read back whole on its medium. It found a fault of the store's mount, fixed with it (Names, History, above);
- `rights_come_from_the_badge`: `copy` with the publish right, `copy-stat` with any;
- deliberate changes of `copies.rs`, one at a time on a copy of the file outside git: each was caught by at least one of these tests, which shows they can fail, not that they find every fault.

On the platform, the QEMU `store` suite (x86 and aarch64, `store_copy_disk_check`) boots with two blank 8 MiB VirtIO disks beside the boot disk, written in place, so that damage made on the host reaches the guest's next read. Through `blocks` it checks:
- the store and its copy both mounted (`COPY MOUNTED`), on the two disks `init` lends in slots 2 and 4;
- a name's copy turned on replying `COPIED`, then its next publication `COPIED`; `copy stat` with one name and none behind;
- one sealed record damaged on each disk while the machine runs, two chunks of the name's first version, which both disks keep in its history: `blocks scrub` finds the store's, the log shows each repaired from the other disk and a collection after the repairs, and the next scrub finds nothing on either;
- one chunk damaged on each disk before a reboot, each the last chunk of one of the two versions: after the reboot both disks are found as the store's, the copy's catch-up puts its missing block back (`COPIED obj VERSION 2 ON THE COPY, 1 BLOCKS PUT THERE`), a read of the first object returns it whole from the copy (`FROM THE COPY`), and after two scrubs the last is clean on both disks;
- turned off, the name's next publication replying `FLUSHED`, and the copy holding no name.

The damage is made by the test, to one bit of a record's sealed bytes between writes and reads; it is not a failure of a real device, and both disks are files on one host disk (MC-12.1). Not tested on the platform: a copy that refuses a step (its writes refused), and a domain's name refused; these are host-tested. A name only the copy holds is, since 309-STO-0041 (below).

## A store rebuilt from its copy — host-tested, run in QEMU on x86 and aarch64 (309-STO-0041)

A store whose medium was lost gets back, from its copy on the second medium, the names that had a copy (`blockstore/src/copies.rs`, the fourth step of [309](../../issues-done/309-copies-scrub-and-repair.done)). A tool says first what the copy would bring back and what it lost. The store's layout stays 4; `blockstore.wit` goes to 2.6, a minor version that adds and changes nothing.

- **The copy's names listed:** `copy-names(after)` (2.6), with the read right (`BADGE_GET`), never through a domain's client (`rights`, logged), as `copy` is. Up to 16 of the names the copy holds with a root, in byte order, each after `after`, asked for page by page as `names` is. Each comes with the copy's version, its root, and how it stands against the store:

  | `standing` | When |
  |---|---|
  | `in-step` | the store's current version has the same root |
  | `behind` | another root in the store, or a removal there |
  | `only-on-copy` | the store does not hold the name at all (its version 0) |
  | `unknown` | the store is not mounted |

  `no-copy` without a copy; without one and with the store unmounted, the store's own error, as to every request.
- **A name brought back:** `rebuild(name, check)` (2.6), with the publish right (`BADGE_PUBLISH`), never through a domain's client.
  - **Only a name only on the copy**, at the copy's current version. Any other is refused with `conflict`: a name the store holds is never overwritten, and a removal never undone. A name the copy holds no root of is `not-found`; in a memory domain the store is `invalid`, as `copy` is.
  - **The walk.** The copy's root is walked over the copy, by the copy's own walk (`journal::walk`, within its steps). Each block it visits that the store lacks is read from the copy, checked against its CID as every read is, and put in the store. A block the store holds already is not read from the copy.
  - **The publication.** The store's name is published at the copy's root by compare-and-swap from version 0, so at version 1, charged to the owner the copy's version records. The name is then in step with its copy, which is not written. The reply is `copied`.
  - **A block the copy lacks that the store holds already** (put by another writer, or by another name's rebuild) does not stop the walk: the name is published, but it is behind on the copy and the reply is the store's class. The next catch-up puts the block on the copy.
  - **With `check`** nothing is put or published, and the reply counts what a rebuild would visit and put. A check counts each visit: a block an object holds twice, a file twice in a directory, is counted twice, where the rebuild puts it once.
  - **A name the copy holds incomplete or damaged.** A walk that meets a block the copy lacks, or holds damaged, stops there. The reply says `incomplete` or `corrupt` and the CID it stopped at, and nothing is published. The blocks it put before are retained by nothing: kept for their lease, then freed by the next collection. The name stays on the copy as it is.
    - A block whose record a mount found damaged is not in the copy's index, so the walk finds it lacking: `incomplete`. A sealed record that does not open does not say which block it held.
    - A block found damaged by a read leaves the copy's index, as with `get`, so the next walk finds it lacking too: `corrupt` the first time, `incomplete` after.
    - Where the walk reads a node itself before it visits it (a journal's older entries, a directory or journal root), the CID is the one the walk could not read (`Store::unread`).
  - **The reply** (`rebuilt`): the name's version in the store (0: nothing published), the blocks visited, those the store lacked when visited and those put, how far the publication reached, how the walk ended (`walked`: `whole`, `incomplete`, `corrupt`) and the CID it stopped at.
  - **The log:** `[BLOCKSTORE] REBUILT <name> VERSION <v> ROOT <cid> FROM THE COPY, <n> BLOCKS PUT`, or `[BLOCKSTORE] NOT REBUILT <name>: <error>`, with ` AT <cid>` where the walk stopped. A check is not logged: it writes nothing.
- **A medium the store cannot mount** (305-STO-0008: another file system, a damaged superblock, another layout, a key no keeper holds). The copy is mounted beside it read-only, so that it says what it would bring back:
  - its medium goes through a gate (`copies::Gate`), shut while the store is not mounted: every write and flush on it is refused, and the copy's own store sees a read-only medium. Its mount writes nothing, and a blank medium, which a mount would format, is not mounted at all (`[BLOCKSTORE] NO COPY YET: ITS MEDIUM HOLDS NO STORE TO READ, AND NONE IS MADE BESIDE A STORE NOT MOUNTED`);
  - it is logged `COPY READ BESIDE A STORE NOT MOUNTED: <n> NAMES ON IT, NOTHING WRITTEN THERE UNTIL THE STORE MOUNTS`. No catch-up, scrub, repair or collection runs, as nothing does without a store;
  - `copy-names` answers with every name `unknown`, and `rebuild(name, true)` with the store lacking every block. `rebuild(name, false)` is refused with `device`;
  - once a `recover` mounts the store (308-STO-0028), the gate opens and the copy's names are compared with the store's as a mount compares them (`COPY MOUNTED: …`), so the copy is kept in step from then on;
  - the store still never formats a medium it cannot mount. Making it blank is a person's act: on QEMU, a blank disk put in its place.
- **The tool:** `blocks rebuild [--check]`. It lists the copy's names with `copy-names`, page by page. Each name gets a line:

  | Line | When |
  |---|---|
  | `IN STEP <name>` | in step |
  | `BEHIND <name>` | behind |
  | `WHOLE <name> VERSION <v> ROOT <cid>: <b> BLOCKS, <m> TO PUT` | `--check`: only on the copy (or the store not mounted), and `rebuild(name, true)` found it whole: `<v>` is the copy's version, `<b>` the blocks visited, `<m>` those the store lacks |
  | `REBUILT <name> VERSION <v> ROOT <cid> (<CLASS>): <m> BLOCKS PUT` | without `--check`: found whole, then `rebuild(name, false)`; `<v>` is the store's version |
  | `NOT REBUILT <name> VERSION <v> ROOT <cid>: <error>` | found whole, and the rebuild refused (`Device` beside a store not mounted) |
  | `LOST <name> VERSION <v> ROOT <cid>: INCOMPLETE AT <cid>` (or `CORRUPT`) | the check's walk stopped there |

  It ends with `CHECKED` (or `REBUILT`) `<n> NAMES ON THE COPY: <w> WHOLE, <l> LOST, <s> IN STEP, <b> BEHIND`, `<n>` the names the copy holds; beside a store not mounted, `THE STORE IS NOT MOUNTED: NOTHING IS BROUGHT BACK INTO IT UNTIL IT MOUNTS`; and always `NOT ON THE COPY, SO NOT SAID HERE: NAMES THAT HAD NO COPY, VERSIONS OLDER THAN EACH NAME'S ON THE COPY, PINS, AND CHANGES THE COPY WAS BEHIND ON; THESE CANNOT BE TOLD FROM WHAT NEVER EXISTED`.
- **What it does not give:**
  - **what the copy never held:** a name that had no copy; the store's history older than each name's current version on the copy; pins; a change the copy was behind on when the store was lost. The tool cannot tell these from what never existed, and says so;
  - **no older version of the copy** is tried when its current one is incomplete, though the copy keeps older ones;
  - **a lost name stays on the copy**, and while it does, the copy's collections are refused, since a name there lacks a block, and its damaged record is not freed: `blocks copies <name> off` removes it there;
  - **no erasing of a medium the store cannot mount**, and no rebuild into one: the copy is read beside it, and nothing is brought back until it mounts;
  - **no pairing check** (309-STO-0040): a copy from another store would be rebuilt into this one;
  - **no rebuild of a name the store holds**, behind or removed there: a catch-up brings the copy to the store's version, never the other way.

Evidence: host tests of the logic on simulated media with the stand-in cipher (`tests/blockstore_host.rs`); tests are not proofs (MC-12.2), and every damage below is made by the test.
- `a_store_whose_medium_is_replaced_by_a_blank_one_is_rebuilt_from_its_copy_and_found_again_after_a_remount_of_both`: an object of a node and three chunks at the copy's version 2, a directory of two files, and one of its files alone under a name of its own, all with copies, and a name without one. The store's medium replaced by a blank one: the three names logged only on the copy and listed `only-on-copy` with the copy's versions and roots. Each rebuilt at version 1, charged to the copy's owner: 4, 3 and 0 blocks put (the file both hold is put once), every block equal on both media, nothing written on the copy, `copied`, then listed `in-step`; the object's older version and the name without a copy not brought back. The rebuilt name's next publication reaches the copy. After a remount of both, everything is found again in step;
- `a_check_puts_nothing_and_counts_what_the_rebuild_then_puts`: an object of a node and four chunks, one of them in the store already: a check, twice, visits 5 and counts 4 lacking, and the store's stats, both media's writes and the log do not change; the rebuild puts those 4. A directory holding one file twice: the check counts 3, the rebuild puts 2. A name rebuilt is then refused;
- `a_name_the_copy_holds_incomplete_or_damaged_is_reported_with_its_cid_and_not_published_and_its_blocks_are_freed_by_the_next_collection`: a chunk damaged on the copy before its mount: the check and the rebuild stop `incomplete` at its CID after 4 visits, the rebuild having put 3 and published nothing. A journal's older entry and a directory's root damaged on the copy while it runs, each read by the walk before it visits it: `corrupt` at its CID, then `incomplete` once the read took it out of the index. The names stay on the copy; the 5 blocks put are kept by their lease, then freed by a collection;
- `a_block_the_copy_lacks_that_the_store_holds_leaves_the_rebuilt_name_behind_until_caught_up`: published, behind and replying the store's class; a catch-up puts the block on the copy;
- `a_name_in_step_behind_or_held_by_the_store_is_refused_with_conflict_and_nothing_is_written`: in step, another root and a removal in the store are each refused `conflict`, checked or not; a name the copy does not hold `not-found`, one no store takes `invalid`; a check is not logged, a rebuild's refusal is; neither medium is written;
- `a_memory_domain_gets_neither_the_copys_names_nor_a_rebuild`: in a domain both are `invalid`; in the volume's domain the rebuild goes on;
- `beside_a_store_that_cannot_be_mounted_the_copy_is_read_its_names_listed_a_check_answered_and_a_rebuild_refused_with_device`: a store whose key keystore lost is not mounted and not written; its copy, through a shut gate, mounts without a write, its names listed `unknown`, a check counting every block lacking, the rebuild refused `device` and logged; the copy's own collection, put and publication refused `read-only`, its medium's bytes, writes and flushes unchanged; a blank medium beside it is not formatted. The key given back, the store mounts, the copy attached: the change it had not taken is behind and caught up, and a publication replies `copied`;
- `the_copys_names_are_listed_16_at_a_time_each_page_after_the_last_name_of_the_one_before`: 43 names in pages of 16, 16 and 11, in byte order, in step, behind and only on the copy as they stand; after a name the copy does not hold, from the next; after the last, none; `invalid` for a byte no name has;
- `random_changes_copies_damage_and_remounts_match_a_model_of_both_media`, now 32 rounds: every fourth round the store's medium is lost, replaced by a blank one, and most of the names only the copy holds are checked, or not, and rebuilt at the next mount; rebuilds and checks of any name go on among the steps. Each is checked against the model: refused, or the blocks visited and those lacking counted as above, and the name then held by the store at version 1. After each step the copy's names are listed and each one's standing checked against the model;
- `rights_come_from_the_badge`: `copy-names` with the read right, `rebuild` with the publish right;
- deliberate changes of the rebuild code, one at a time on a copy of the files outside git, each caught by at least one of these tests, which shows they can fail, not that they find every fault: 18 of them, each named by what it changed and caught by: a name the store holds not refused (the conflict and check tests, the random model); no `device` without a store (the unmountable store); every block read and put, held by the store or not (the check, the blank medium, the block the copy lacks); a block the copy lacks said `corrupt` (incomplete or damaged); a check that puts (the check, incomplete or damaged, the random model); the version charged to no owner (the blank medium); the walk's own failed read said to stop at the root, and the store not remembering it (incomplete or damaged: the journal's older entry); a name whose block the copy lacks not put behind, or said `copied` (the block the copy lacks); the gate letting writes through, and a gate left open beside no store staying open (the unmountable store); a page of 17 (the paging test); a name behind listed in step (four tests); `attach` comparing nothing (the unmountable store); a check logged (four tests); a stopped walk publishing (incomplete or damaged); a memory domain's store used (the domain test).

On the platform, the QEMU `store` suite (x86 and aarch64) runs `store_rebuild_check` after the copy check, on its own machine with two blank 8 MiB VirtIO disks beside the boot disk, written in place. Through `blocks` it checks:
- two names published with copies, an object of 40000 bytes and one of 50000 that share their first two chunks, both `in-step`;
- then, while the machine is idle, the third chunk of the second name damaged on the copy's image and the store's image overwritten with zeros, and a reboot: `init` finds the store's disk blank and the copy's the store's; the store formats its disk, and the copy logs both names only on it;
- `blocks rebuild --check`: `LOST gone … INCOMPLETE AT <cid>`, the CID the suite computes for that chunk; `WHOLE keep …: 4 BLOCKS, 4 TO PUT`; `CHECKED 2 NAMES ON THE COPY: 1 WHOLE, 1 LOST, 0 IN STEP, 0 BEHIND`; the store's counts unchanged and nothing published;
- `blocks rebuild`: the lost name as before, `REBUILT keep VERSION 1 ROOT … (COPIED): 4 BLOCKS PUT`, logged; the object read back by its pattern; then `IN STEP keep`;
- through a memory domain's client, `rights` (where a domain can be made);
- after another reboot, both disks found as the store's, the name at the same root and read back by its pattern.

The damage and the blank disk are made by the test, between writes and reads, and both disks are files on one host disk (MC-12.1). The machine reboots rather than starts again, because `keystore` keeps the copy's volume key on the boot volume, whose snapshot overlay outlives a reboot and not QEMU. Not tested on the platform: a store that cannot be mounted beside its copy, a name behind refused, and a block the copy lacks that the store holds; these are host-tested.

## A journal, checked from its head — host-tested, run in QEMU on x86 and aarch64 (312-STO-0034)

A journal is an ordered record under a name, which a reader checks whole from its head (`libmind/src/journal.rs`), the first step of [312](../../issues/312-the-minds-memory.md): every entry there, none reordered, changed or dropped, and the record never rewritten from an older point. It is a client's use of `put`, `publish`, `snapshot` and `get`: `blockstore.wit` and the store's layout do not change.

- **An entry** is one DAG-CBOR block `{"v": 1, "seq", "boot", "kind", "prev", "clock", "source", "content", "calendar", "monotonic"}`, its keys in DAG-CBOR's order:
  - `seq`: its sequence, from 1;
  - `boot` and `monotonic`: the boot it was written in, as the journal counts them from 1 (below), and the monotonic time in it, in nanoseconds;
  - `calendar` and `clock`: the calendar time, seconds since 1970-01-01 UTC, and the clock it came from: 0 none (the time is then 0), 1 the clock chip, 2 the network;
  - `source`: the program that wrote it, as the writer gives it; `kind`: a short label. Each is 1 to 32 bytes of `!` to `~`;
  - `content`: a link to the content, a file or a directory (above);
  - `prev`: a link to the entry before, a `dag-cbor` CID, absent for the first and only for it.

  `"v"` is the schema's version, and another is refused (MC-4.13). **One encoding:** an entry is accepted only if re-encoding it gives exactly its bytes; at most 250 bytes.
- **Appending** (`journal::append`): the writer puts the content first, a file of any size as an object. The append reads the name's version and head, puts an entry of sequence `version + 1` linking the head, and publishes it by compare-and-swap from the version read. So an entry's sequence is the name's version it is published at.
  - Of two writers that read the same head, the second is refused with `conflict`, nothing published; it reads the new head and appends after it, so the journal never forks. The entry it put is retained by nothing, and a collection frees it once its lease (60 s) has ended.
  - **The boot** is counted by the journal: the head's, or the next when the head's monotonic time is past the writer's (its clock started again); the first entry's is 1. No boot identity exists on the platform (no system call names the boot), so a new boot whose clock has already passed the head's is counted as the same boot.
  - A head the check would refuse at once (no entry of this schema, or not at its version) takes no entry, and nor does a removed name.
- **The check from the head** (`journal::check`) reads the name's version and root, then each entry down to the first, within a bound of steps (`blocks` gives the store's own: four for each block its index holds, and 64). Each entry is read and checked against its CID and the schema. It refuses:
  - a sequence that is not the name's version at the head, or not one less than the entry above it below; the schema puts the first, and only it, at 1 with no link before;
  - a block missing (`missing`) or corrupt (`corrupt`), and a block that is no entry of this schema (`format`, `version`);
  - a boot that goes back, and a monotonic time that goes back within one boot (`time`); the same time twice is not going back.

  So a head published again at a later version, a rewrite, is refused, as a checkpoint's epoch is (313-STO-0033), and so is a journal whose name was removed and published again (`removed` while it stays removed). The check never orders by the calendar time, and does not read the contents.
- **The block store retains a journal whole.** It takes an entry as a typed `dag-cbor` block, as it takes `dag`'s nodes and `tree`'s directories. Its walk (`journal::walk`; `tree::walk` for any other root) follows the links to the entries before one after another, never nested: `tree` bounds a path at 32 directories, and a journal is longer.
  - Each entry is visited, then its content's blocks (`tree::walk`), all within one walk's steps. An entry linking one that is not one less in sequence, or no entry, and a content that is an entry, are out of shape (`invalid`).
  - So a publication, a pin or a commit of a head needs every entry and every content (`incomplete`), and a collection keeps every block a kept head retains. `dag::complete` is an object's; the store's own check of a publication is this walk.
  - An owner is charged the bytes of the contents under each version its name keeps, at its root, as for directories (above): the four heads a journal's name keeps are charged about four times its contents. A mount reads them again (`journal::size`).
- **Copies** (309-STO-0031) walk a journal as the store does: `blocks export` writes every entry and content in the walk's order, and a check of the copy reads them all. An import publishes the name at the versions of the store it comes into (1 into an empty one), so `check` there refuses the head as one published again: a journal's order is checked in the store it was written in.
- **A memory domain** (308-STO-0023): a journal in a domain is written through the domain's client (`blocks -d <id> journal …`), is the domain's alone, and is forgotten with it. Nothing new is needed for this.
- **`blocks journal`:**
  - `append <name> <kind> <text>`: the text's words joined by one space, as a `raw` block; `append <name> <kind> -f <file>`: a file, as an object. The source is `blocks`; the calendar time is the clock chip's, read through the `rtc` service, or none. It prints `APPENDED <name> SEQUENCE <s> BOOT <b> ENTRY <cid> CONTENT <cid> (<durability>)`. A conflict prints `CONFLICT <name>: ANOTHER WRITER APPENDED; READING THE HEAD AGAIN`, and the append is made after the new head, up to 8 times;
  - `check <name>` prints `CHECKED <name>: <n> ENTRIES, SEQUENCE <first> TO <last>, AT VERSION <v>: WHOLE`, or `REFUSED <name> AT VERSION <v>: SEQUENCE <s>, AFTER <n> ENTRIES CHECKED: <error>`;
  - `list <name> [n]` prints the last n entries (10 if not given), newest first, each checked as `check` checks it, on two lines: `ENTRY <seq> <cid> BOOT <b> MONOTONIC <ns> CALENDAR <s> <NONE|CHIP|NETWORK>`, then `  SOURCE <source> KIND <kind> CONTENT <cid>`, with `TEXT <text>` for a `raw` content of at most 96 printable bytes; then `<k> ENTRIES OF <name> AT VERSION <v>, NEWEST FIRST`;
  - with `-d <domain>`, as every command of the tool.
- **What it does not give:**
  - **The source and the calendar time are the writer's claims,** and so is the boot the journal counts. The store records the publisher's badge in a name record, but `history` does not return it. A source attested by the store is a later step, on the wire.
  - **A writer with the name's rights can append anything.** The check shows the order and completeness of what was published, not that the entries are true (MC-4.12).
  - **A journal is as durable as its store** (the classes above: on the RAM disk nothing outlives a reset). A copy carries it whole, since a copy is of a name's object; in another store it is checked at that store's versions (above).
  - **Each append walks the whole journal,** as every publication walks its root (MC-4.4), and a collection walks each version a name keeps: their work grows with the journal's length. A journal is bounded by one walk's steps, two an entry with a content of one block.
  - **The check does not read the contents:** a content's block damaged after its entry was published is found by a read of it or a scrub.

Evidence: host tests of the code, not a proof (MC-12.2).
- `tests/journal_host.rs`, through the store's own logic on a simulated medium:
  - two entries are byte for byte what the reference library `dag-cbor` 0.3.3 encodes, with the same CIDs. Numbers in every integer form and labels of 1 and 32 bytes encode as the schema written in the test does, and read back. Every byte of an entry changed to every other value is refused or reads as exactly what it encodes. Every cut, another version, keys out of order, missing or extra, longer forms, a sequence and a link before that disagree, a link before as `raw`, a clock of none of the three, a calendar time without a clock, and labels that are none are refused;
  - 1000 entries appended check whole, also after a remount and after a collection once the leases ended, which frees a block nothing retains and keeps every entry. The last five are listed newest first with their contents, and the owner is charged the contents of the four kept heads, also after the mount;
  - the check refuses, each with the place it stopped:
    - an entry's block erased on the medium (`missing`, the entries above it checked); the store then takes no entry after it, and no collection frees anything;
    - an entry changed, another entry with the same links: answered by a store that lies (`corrupt`), or written into the store with the entries above it again and published (its head's sequence);
    - an older head published again, and the last head published again;
    - a name removed (`removed`), then published again with a new journal's first entry, or with the old head;
    - a monotonic time that goes back within one boot, and a boot that goes back. A new boot's clock starting again is not refused, and an append counts it;
  - of two writers that read one head, the second is refused with `conflict`, nothing published, and appends after the first's entry; the check passes, and the refused entry is freed by a collection after its lease;
  - a journal in a memory domain is the domain's alone, mounts again, and is gone with the domain's forgetting, its records collected, the volume's journal of the same name whole;
  - an append stopped after each of the store's calls (the content's put, the head, its entry, the entry's put, the publication), and on a medium stopped at each write of it, not made or only its first sector: the check passes on whatever the head is, the old one or the new;
  - the store's walk visits each entry, then its content, from the head down, over 96 entries (three times `tree::DEPTH`), and one step fewer stops it. A link to an entry not one less, a link to a directory, and a content that is an entry are `invalid`; an entry of another version is no block the store takes; an entry whose content is not stored is not published; a journal whose walk takes more steps than the store gives is refused;
  - an export and the copy's check carry every entry and content. Imported into an empty store, the name is at version 1 there and the check refuses its head; where it was written, it checks whole;
  - what a store that lies answers is refused: a gap, an entry of another version below the head, an entry's bytes under a `raw` CID, a directory, bytes not the block's. A check reads within its steps or is refused at the entry past them; a listing reads as many as asked.
- The QEMU `store` suite (x86 and aarch64): `blocks journal append` three times, each at the name's next version. `check` and `list` print them, each entry's CID the one the suite computes from the schema and the times listed, linking the one before. Once the leases have ended, a collection frees a block nothing retains, and the journal still checks and lists whole. The second entry published again by hand with `blocks publish` is refused by `check` (`Sequence(2)` at version 4), and nothing is appended after it. Where a memory domain is made (aarch64), a journal in it is the domain's alone and is gone with its forgetting.

## A text index derived from a journal — host-tested, run in QEMU on x86 and aarch64 (312-STO-0035)

A text index tells which entries of a journal hold a word without reading every content (`libmind/src/index.rs`), the second step of [312](../../issues/312-the-minds-memory.md). It is derived (MC-4.12): built only from what the journal holds, it may be dropped and rebuilt, and a rebuild gives the same index. It is not authority: each answer is checked against the journal. It is a client's use of `put`, `publish`, `snapshot` and `get`: `blockstore.wit` and the store's layout do not change.

- **An index is an object of `tree`** (above), published under a name of its own by compare-and-swap, `index/<journal>` unless one is given. So the store retains and walks it, a copy carries it, and its owner is charged the bytes of its files. Its root holds two entries:
  - `manifest`: a file, one `raw` block, holding a canonical DAG-CBOR map `{"v": 1, "seq", "head", "kind", "terms", "entries", "journal", "postings", "tokenizer"}`, its keys in DAG-CBOR's order: the journal's name; the sequence and the CID (a link) of the head it was built from; `kind` `"text"`; `tokenizer` 1; how many entries hold a term, how many terms and how many postings. At most 208 bytes, and accepted only if re-encoding it gives exactly its bytes. `"v"` is the schema's version: another is refused by its number (`version(n)`), and so is another tokenizer (`tokenizer(n)`); another kind is refused (`kind`). The manifest is a file, whose bytes the store does not read: an index does not retain its journal.
  - `terms`: a directory for each first byte of a term, each holding a directory for each first two bytes (for a term of one byte, that byte), each holding a file for each term: `terms/a/ab/abacus`. A term's file is one `raw` block, a canonical DAG-CBOR array of the sequences of the entries whose content holds it, ascending, each once.
- **The tokenizer, version 1:** a term is a run of ASCII letters and digits in a content, lowercased, 1 to 32 bytes; a longer run is cut into pieces of 32, and every other byte separates. **Text beyond ASCII is not indexed:** its bytes separate, so `naïve` gives `na` and `ve`. A content is a file, read whole with every block checked against its CID; a directory as a content holds no term, and its names and files are not read.
- **Building** (`index::build`) reads the index name's version, then the journal from its head down to the first entry, each entry checked as `journal::check` checks it and its content tokenized as it is met. It then writes each term's file, the directories and the manifest in the terms' order, and the root, and publishes the root from the version read. A journal that does not check has no index (its check's refusal). The same head gives the same bytes, so the same root, and a name that holds it already is not published again. A build whose name another writer published since it was read is refused (`conflict`) and publishes nothing.
  - **The work area:** `libmind` has no heap, so the terms, their bytes and their postings lie in a work area the caller gives. `blocks` gives 4096 terms, 64 KiB of their bytes, 32768 postings, and three blocks to read and write through, about 430 KiB. A journal past it, or a sequence past 2³² − 1, is refused (`full`) before anything is put, and nothing is published.
  - **A directory is one block** (above): a shard of more terms than a directory holds is refused (`too-large`), about 320 terms of 5 bytes sharing their first two bytes, or 200 of 32. So is a term's postings over a block, a term in more than about 5500 entries (not tested). Nothing is then published; the files put are retained by nothing and collected once their lease has ended.
- **Stale or current** (`index::state`): the journal is checked whole from its head, as a build reads it. The index is current when its head is the journal's head, and stale when the journal's entry at the index's sequence is the index's head: the journal grew by the entries above it, and by how many is told. Anything else is refused: the journal does not check (its check's refusal), its entry at that sequence is another (`another`), or it is shorter (`shorter`): an index of another journal, or of one rewritten. An index whose manifest names another journal is refused too (`another`).
- **Search** (`index::search`): the word is tokenized as a content is, into 1 to 8 terms (else `word`), and the answer is the entries whose content holds every one. The postings of the word's terms are intersected; then the journal is read from its head as `state` reads it. Each entry above the index's head is scanned, its content tokenized. Each answer below it is checked, its content read and tokenized again: an entry that does not hold the terms is a false answer, and the index is refused (`false(n)`). The answers are given once all are checked, newest first. So an answer is the exact scan's.
- **The exact scan** (`index::scan`) is the answer without an index: the journal checked whole, every content tokenized. The host tests hold every search to it.
- **The check** (`index::verify`): the index rebuilt in the work area from its own head, nothing put, and its root compared with the one published: equal; stale by n, equal to its rebuild from its own head, the journal grown by n entries since; or differs. A check tells what a search cannot: a posting or a term dropped, or a manifest whose counts are not its postings'.
- **A memory domain** (308-STO-0023): an index lives in the domain of the client that builds it, beside its journal, and is forgotten with the domain.
- **`blocks index`:**
  - `build <journal> [name]` prints `INDEXED <journal> TO SEQUENCE <s>: <e> ENTRIES WITH TERMS, <t> TERMS, <p> POSTINGS`, then `PUBLISHED <name> VERSION <v> ROOT <cid> (<durability>)`, or `UNCHANGED <name> VERSION <v> ROOT <cid>` when the name holds that root already;
  - `search <journal> <word> [name]` prints the answers newest first, 16 to a line, as `SEQUENCES <s> <s> …`; then `INDEX <name> OF <journal> AT SEQUENCE <s>: CURRENT`, or `…: STALE, LACKS <n> ENTRIES, SCANNED`; then `FOUND IN <n> ENTRIES OF <journal>, NEWEST FIRST, EACH CHECKED AGAINST THE JOURNAL`;
  - `check <journal> [name]` prints `INDEX <name> OF <journal> AT SEQUENCE <s>: EQUAL`, `…: STALE <n>` or `…: DIFFERS`;
  - a refusal prints `blocks: index <command> <journal>: <error>`, or for a journal that does not check the `REFUSED …` line of `blocks journal check`; with `-d <domain>`, as every command of the tool.
- **What it does not give:**
  - **Not vectors,** and no term beyond ASCII: a word in another script is not found. Vectors wait for `NRL`'s embeddings.
  - **Bounded** by the work area, and by a block for a shard and for a term's postings: a journal past them has no index. An index on the medium that grows is [310](../../issues/310-the-index-on-the-medium.md)'s.
  - **Not faster to check:** every search reads the journal's entries from its head to the first, as its check does, and the contents of the entries above the index's head and of every answer. An index saves the tokenizing of the entries no term of the word names; it does not make a journal's check shorter.
  - **A search cannot tell a posting dropped:** it answers fewer entries than the scan, and only a check, which tokenizes every content again, tells it.
  - **No rights:** an index lives in the store and the domain its journal does; it grants nothing and reads nothing its reader's client cannot (MC-4.6).
  - **Not proof that it is current** (MC-4.12): whether it is current is read from the journal at each use, never from the index.

Evidence: host tests of the code, not a proof (MC-12.2).
- `tests/index_host.rs`, through the store's own logic on a simulated medium:
  - a manifest and a postings file are byte for byte what the reference library `dag-cbor` 0.3.3 encodes, and a small index written in the test from the layout has the root the reference library gives. Every byte of a manifest changed to every other value is refused or reads as exactly what it encodes; every cut, keys out of order, missing or extra, a longer form, a head that is no entry's, more entries than the sequence and a journal's name the store does not take are refused. Another version, kind and tokenizer are refused as such, by `decode` and by a search, a state and a check of an index whose manifest it is;
  - the tokenizer gives what its rule, written in the test, gives, for every byte and runs of every length to 100;
  - an index of 240 entries of seeded words is built twice to one root, the root of the index written in the test from a model of the journal, also after a remount, and after a collection once the leases have ended, which frees a block nothing retains and keeps every block of the index;
  - every search (each term the journal holds, in other cases, words it does not hold, words of several terms) answers what the exact scan and the model answer; a word of no term, or of more than 8, is refused;
  - after 7 appends, a directory and an object of two chunks among them, the index is stale by 7, and every search still answers the scan, the entries above its head among the answers; the check says stale by 7; rebuilt, it is current and equal, its root the one written in the test;
  - refused: an index asked of another journal; copied into stores whose journal of the same name is another, longer (`another`) or shorter (`shorter`); a journal rewritten from an older point, for a build (which publishes nothing), a state, a search, a check and a scan; a journal read past its steps; a journal with an entry's block erased on the medium;
  - an index changed by hand and published under its name: a posting added, or a term no entry holds, is a false answer refused by the search and told by the check (differs); a posting or a term dropped is answered short by the search and told by the check; a miscounted manifest is told by the check; a manifest naming another head is refused; postings files unsorted, twice, of 0, past the sequence, empty, longer or with a byte after, and roots out of the layout, are refused (`format`);
  - one short of each bound of the work area: refused, nothing put and nothing published; at the bounds, built. 400 terms sharing their first two bytes: refused (`too-large`), nothing published; 300 built;
  - a word across a chunk's end of a content of three chunks is one term; a directory as a content holds none;
  - an index in a memory domain is the domain's alone and goes with it; of two builders, the second publishes nothing (`conflict`); a journal with no term has an index of none; a store that lies about a content or about the index is not believed (`corrupt`).
- The QEMU `store` suite (x86 and aarch64): `blocks journal append` four times, then `blocks index build`, whose root is the one the suite computes from the layout (written from the DAG-CBOR specification, checked against the reference library's root for a small index); `search` prints the sequences of each word's entries newest first, as the notes hold them, and `check` says `EQUAL`. After another append, `search` says the index lacks one entry and still finds it, and `check` says `STALE 1`; rebuilt, `check` says `EQUAL`, and once the leases have ended, after a collection that frees a block nothing retains, the same. A journal that does not check has no index, and an index asked of another journal is refused. Where a memory domain is made (aarch64), an index in it is the domain's alone and is gone with its forgetting.

## Assertions about objects — host-tested, run in QEMU on x86 and aarch64 (312-STO-0038)

An assertion says something about an object without changing it (`libmind/src/assertion.rs`), the third step of [312](../../issues/312-the-minds-memory.md): a claim with its type, the version of the type's meaning, its source and its time (MC-4.6), kept apart from the object's content (MC-4.12). A title or a tag written into an object's content would change its CID, so it would no longer be the same object, and nothing would tell who said it. It is a client's use of the journal (above): `blockstore.wit` and the store's layout do not change.

- **An assertion** is one canonical DAG-CBOR map `{"v": 1, "type", "basis", "value", "interp", "subject", "retracts"}`, its keys in DAG-CBOR's order:
  - `subject`: the CID it is about;
  - `type`: what is said, a label as a journal's kind is, 1 to 32 bytes of `!` to `~`;
  - `interp`: the version of that type's meaning, a number (MC-4.6); `blocks` writes 1;
  - `value`: a text of at most 1024 bytes of UTF-8, or a CID;
  - `basis`: absent, or the CID of what it was derived from (a model, an index, another object);
  - `retracts`: absent, or the CID of an earlier claim of the same source that it withdraws.

  A **claim** has a `value` and no `retracts`; a **withdrawal** has `retracts` and no `value`, and names an assertion's block (`raw`). `"v"` is the schema's version: another is refused by its number (`version(n)`, MC-4.13). **One encoding:** an assertion is accepted only if re-encoding it gives exactly its bytes; at most 1182 bytes.
- **A mention, not a retention** (MC-4.5): `subject`, `basis`, `retracts` and a CID value are the CIDs' binary forms as byte strings, never links (tag 42). No walk follows them, the store's, a copy's or a DAG-CBOR tool's: an assertion never retains its subject, and a subject collected leaves its assertions in place. The subject need not be held by the store at all.
- **Kept in a journal:** an assertion is a `raw` block, the content of a journal entry of kind `assertion`, appended by `journal::append`. The journal gives it its order, its time and its `source`, the writer's label as the journal records it. So an assertion's provenance is its entry's, checked from the head as every entry is. The journal retains each assertion through its entry, as any content, and its owner is charged the assertion's bytes.
  - **Why `raw`:** the store takes a `dag-cbor` block only of the schemas its walk reads (a node, a directory, a journal's entry), and takes a `raw` block as a file of one chunk whose bytes it never reads. An assertion's CID is so of the codec `raw`, and its bytes are canonical DAG-CBOR all the same. Each read checks them against that CID.
- **Adding** (`assertion::add`): the claim is put as a `raw` block, then appended by compare-and-swap; a writer that lost the race to another is told (`conflict`), nothing appended. A withdrawal is not added this way, only by `retract`.
- **Retracting** (`assertion::retract`): the journal is checked whole from its head and every assertion in it read; then a withdrawal of the claim, with the claim's subject, type and interpretation, is appended. Only the source's own claim that stands is withdrawn: one the journal does not hold (`not-found`, a withdrawal among them), another source's (`another`) or one withdrawn already (`withdrawn`) is refused, and nothing is appended.
- **Reading** (`assertion::about`): the journal is checked whole from its head, as `journal::check` checks it; each entry of kind `assertion` has its assertion read and checked against its CID and the schema, and entries of other kinds are skipped. The answer is the claims about the subject, newest first, each with its entry's sequence and source, without those a later withdrawal of their own source names.
  - **A withdrawal by another source withdraws nothing:** it is that source's own record, and the claim stands. A withdrawal withdraws only the claims before it, so the same claim said again after it stands. A withdrawal is not itself an answer.
  - **Refused:** a journal that does not check (its check's refusal), an assertion's block missing (`missing`) or not its CID's bytes (`corrupt`), and an entry of kind `assertion` whose content is no assertion of this schema (`format`, `version(n)`). The answers are given once the whole journal is checked, so a journal that does not check answers nothing.
  - **The work area:** `libmind` has no heap, so the claims and withdrawals of the subject met while reading lie in a work area the caller gives. `blocks` keeps 256, about 20 KiB; past it is `full`, nothing answered.
- **The sources of truth** (MC-4.12):
  - an object's content is its own: its CID names it, and nothing said about it changes it;
  - an assertion with no `basis` is its source's word, and its journal is its record;
  - one with a `basis` is derived: it may be dropped and made again from its basis.
- **A memory domain** (308-STO-0023): assertions live in the journal's domain, written through that domain's client, and go with it.
- **`blocks assert`:**
  - `add <journal> <cid> <type> <text>`: a text value, the words joined by one space; `add <journal> <cid> <type> -c <cid>`: a CID value. The source is `blocks` and the interpretation 1. It prints `ASSERTED <journal> SEQUENCE <s> ASSERTION <cid> ABOUT <cid> (<durability>)`; a conflict prints `CONFLICT <journal>: ANOTHER WRITER APPENDED; READING THE HEAD AGAIN`, and the append is made after the new head, up to 8 times;
  - `about <journal> <cid>` prints each claim newest first, on two lines: `ASSERTION <seq> <cid> SOURCE <source> TYPE <type> INTERP <n>`, with `  BASIS <cid>` if it has one, then `  TEXT <text>` (a text of more than 192 bytes, or with a control character, as `  TEXT OF <n> BYTES`) or `  CID <cid>`; then `<n> ASSERTIONS ABOUT <cid> IN <journal> AT VERSION <v>, NEWEST FIRST`;
  - `retract <journal> <assertion>` withdraws a claim of `blocks` and prints `RETRACTED <assertion> IN <journal> SEQUENCE <s> ASSERTION <cid> (<durability>)`;
  - a refusal prints `blocks: assert <command> <journal>: <error>`, or for a journal that does not check the `REFUSED …` line of `blocks journal check`; with `-d <domain>`, as every command of the tool.
- **What it does not give:**
  - **Not signed.** Appendix B.3 asks for a signature where provenance crosses a trust zone; a local record needs none. The source is the writer's label, as a journal's is, and a writer with the journal's rights may write any source.
  - **Not true:** an assertion says who claims what, not that it holds (MC-4.6).
  - **No rights:** an assertion lives in its journal's store and domain, and reading one about an object grants nothing on the object (MC-4.6).
  - **A scan:** each `about` and each `retract` reads the whole journal and every assertion in it. An index by subject waits for the sizes to ask for it, as the text index did.

Evidence: host tests of the code, not a proof (MC-12.2).
- `tests/assertion_host.rs`, through the store's own logic on a simulated medium:
  - four assertions (a text claim, a CID claim with a basis, two withdrawals) are byte for byte what the reference library `dag-cbor` 0.3.3 encodes, with the same CIDs, and none holds a link. Numbers in every integer form, texts of 0 to 1024 bytes and types of 1 and 32 bytes encode as the schema written in the test does, and read back; the largest is 1182 bytes. Every byte changed to every other value is refused or reads as exactly what it encodes; every cut, a byte after, another version (by its number), keys out of order, missing or extra, a claim with a withdrawal or with neither, a text past 1024 bytes or not UTF-8, a type that is no label, a CID as a link, cut short or of another codec, a withdrawal of a `dag-cbor` block, and longer forms are refused; what makes no assertion is not encoded;
  - `about` answers one subject's claims newest first, with their sequences and sources, the same bytes said by two sources each its own; none of another subject; an entry of another kind holding an assertion's bytes is not one;
  - over a seeded journal of 240 appends by three sources (claims, retractions of any earlier entry, withdrawals written without `retract`, notes), every answer is what a model of the rules written in the test answers, and each retraction is done or refused as the model says;
  - a withdrawal by the same source withdraws, one by another does not; retracting another source's claim, a claim withdrawn already, a withdrawal, an entry, a note and a CID not held is refused with nothing appended; a claim said again after its withdrawal stands, and is withdrawn again;
  - the subject, an object of three chunks, its basis and a CID value, all collected once their names are removed and the leases ended: the store's walk of the journal visits each entry and assertion and nothing they mention; `about` still answers both claims, also after a remount;
  - an assertion answered by a store that lies, with another assertion's bytes, is refused (`corrupt`); one changed on the medium is not held after a mount (`missing`); a journal read past its steps, rewritten from an older point, or with an entry's block erased answers nothing; an entry of kind `assertion` holding a text, another version, a directory or an object refuses the journal's answers;
  - one place short of the work area is `full`, nothing answered.
- The QEMU `store` suite (x86 and aarch64): `blocks assert add` twice about an object, a text and a CID, each assertion's CID the one the suite computes from the schema (written from the DAG-CBOR specification, checked against the reference library); `about` lists both newest first. `retract` withdraws the first, its withdrawal's CID computed too, and `about` lists the other; withdrawing it again, or a withdrawal, is refused. The object's name removed and the leases ended, a collection frees it, and `about` still lists the assertion, the journal whole. Where a memory domain is made (aarch64), an assertion in it is the domain's alone and goes with it.

## Names — run on the platform (302, 303-STO-0003, 0004)

A **name** is a stable entity, its **versions** are the immutable roots it has pointed at, and its **head** is the current one (MC-4.3). The three are kept distinct:
- the root is a CID, immutable;
- the name is 1 to 64 bytes of `A-Z a-z 0-9 . _ / -`;
- the version counts publications from 1.

- **Compare-and-swap.** `publish(name, expected, root)` succeeds only if `expected` is the name's current version (0 for a new name), and returns the new version. Of two publishers that read the same version, the first wins and the second gets `conflict`. Nothing of the refused publication is written, and the loser decides what to do: read the new head, merge, try again. This is the declared protocol for concurrent updates.
- **Only complete roots (MC-4.4).** Before a publication, the store checks with `dag::complete` that every block of the root's object is stored. Every node is read and checked against its CID and the shape, and every chunk is looked up. A missing block is refused with `incomplete` and a tree out of shape with `invalid`. A name therefore never points at data the store has not received.
- **Durability.** Blocks are flushed when they are put, and the name record is flushed before the reply. The durability level is what the device's flush gives; on the RAM disk that lasts until the next reset.
- **Record (layout 4).** Sealed in one sector, written only into a blank sector: `MIND-REF`, layout version (u16), name length (u16), version (u64), the root's CID (36 bytes, zero for a removal), the root of the version before (36 bytes, zero for the first), the name padded with zeros to 64 bytes, the kind (0: the version points at its root, 1: it removes the name), zero, the owner (the publisher's badge, u16), and the SHA-256 of these 160 bytes.
- **History (303-STO-0003, MC-4.5).** A name keeps its newest 4 versions: the current one and up to 3 before it. Each record links the root of the version before it, so the links are explicit on the medium. When mounting, the newest valid version of each name is current and the next valid ones are kept up to 4; a collection frees the records of older versions. `history` lists the kept versions, newest first. Every kept version retains its object (Retention, below).
  - **A removal ends the versions before it:** none older than one is kept, in memory or by a mount, whatever order the mount meets their records in. Before 309-STO-0040 a mount could keep such a version again, from a commit record that stayed because another name kept its own version from it, and lying after the removal on the medium; a collection had freed that version's object, so every collection after the mount was refused with `incomplete`. 309-STO-0040's random model found it; `a_removal_ends_the_versions_before_it_at_a_mount_too` fails without the fix.
- **Removing a name (303-STO-0004, MC-4.8).** `unpublish(name, expected)` is a compare-and-swap like a publication, and writes a version without a root. The name then resolves to `not-found` and retains nothing; it keeps only that version, and a collection frees the records before it. Removing a name deletes a reference, not data: its objects go only when a collection finds that nothing else retains them (another name, a pin, a lease). Publishing from the removal's version creates the name again with the next number.
- **A damaged record.** If the latest record of a name does not open, it is counted as corrupt (`stat`; damaged if its frame is gone) and the version before it is current. That loses a confirmed change on a damaged medium, but the loss is reported, not silent. A name with a copy on a second medium (below) keeps that version there until the mount's comparison finds the name behind: the copy then follows the store to the version before, the lost root kept among the copy's own versions. Nothing takes a version back from the copy yet (MC-4.8).
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
| 8–9 | layout version (u16, 4) |
| 10–11 | domain (u16): 0, the volume's; a memory domain's number; 0xFFFF, the domains' key records (below); 0xFFFE, the recovery records (below, host-tested only) |
| 12–15 | the sectors the record covers (u32) |
| 16–27 | nonce |
| 28–43 | tag |

Then the record and zeros up to the end of its last sector, encrypted. So the frame says how many sectors a record takes, which the layout shows anyway, and not what kind of record it is.
- **The associated data** is the frame's first 16 bytes and the record's first sector number. A record copied to another place does not open there.
- **The nonce** is the first 12 bytes of an HMAC-SHA-256 of the associated data and the sealed bytes, under a key derived from the volume key. A write needs no random source, which a qemu64 CPU and the i7-930 lack. The same nonce comes back only for the same record at the same place, which seals to the same bytes.
- **Keys.** Two keys are derived from the volume key by HMAC-SHA-256: one seals, one makes nonces.
- **The volume key** is made at the first mount of a blank medium, or of a partition `make_usb_image.py` formatted with no key yet. It is random where the processor gives random numbers (RDRAND, RNDR). Elsewhere it comes from the medium's size and the time, and the log says so (`VOLUME KEY MADE WITHOUT RDRAND`).
- **A key kept in the superblock is in the clear.** That is **no confidentiality:** whoever reads the medium reads the key and opens every record. A RAM disk's key is kept there, and so is any key `keystore` does not keep; the service's ready line then says `SEALED, KEY IN THE CLEAR`. Any other medium's key is kept by `keystore` (next section).
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

**Not provided:** confidentiality of a medium whose key is in the clear. What sealing protects and what it does not is in [the threat model](../profile/threat-model.md) (308-STO-0024).

## The volume key kept by `keystore` — run on the platform (308-STO-0022)

A medium that outlives the machine's memory keeps no key: `keystore` keeps it as the store's own secret.
- **Key source 2, kept.** The superblock holds an id of 8 random bytes where the key would be, zeros after it, and the check as before. The key is `blockstore`'s secret `volume-<id in hex>` in `keystore` (`keystore.wit` 1.4's `secret-put` and `secret-get`), sealed there under a key from the device key.
- **The client.** `init` lends the store a secrets client in its slot 3 under a name no loaded program can take, whenever `keystore` runs ([308-KRN-0183](../../issues-done/308-KRN-0183-a-secrets-client-for-blockstore.done), the kernel session's, since round 46). Without it, a new medium keeps its key in the clear, and the log says why (`IN THE SUPERBLOCK: NO SECRETS CLIENT`).
- **Which key is kept.** At the first mount of a blank medium, or of a partition with no key yet:
  - a RAM disk keeps its key in its superblock, as nothing of it outlives a reset and a restarted store must find it;
  - any other medium's key is kept, if it is random and `keystore` keeps it;
  - a key derived without a random source is never kept: it could be found again from the medium's size and the time. `keystore` itself has no device key without one;
  - a key `keystore` refuses (no device key, 16 secrets already) stays in the clear, and the log says so.
- **No conversion.** A medium whose key is in the clear keeps it there; one with a kept key never writes it out.
- **Mounting a kept key's medium:**
  - its key comes from `keystore` and must open the check;
  - without the client, or when `keystore` keeps nothing under the id (another machine's medium, or a secret lost), it is not mounted: `key` on the wire, and the log names the secret (`NOT MOUNTED: ITS VOLUME KEY IS KEPT BY KEYSTORE AS volume-…`);
  - another key under the id does not open the check: `key`.
- **The ready line** says `SEALED, KEY KEPT BY KEYSTORE` or `SEALED, KEY IN THE CLEAR`; a key made at this mount is logged with where it went and why.
- **What it does not give:**
  - where there is no TPM, the device key lies on the boot volume in the clear. So a store disk alone opens nothing, but with the boot volume beside it, it opens. The stick's own store partition (307-STO-0020) is beside it. Where there is a TPM, it seals the device key (351-NET-0006), and the store opens only on that TPM;
  - a recovery key and a passphrase are still to come, with `APP` and `KRN` ([308-STO-0028](../../issues/308-STO-0028-a-recovery-key-and-a-passphrase.md)); the store's part of the recovery key is host-tested and its calls are on the wire (next section), but no program holds a client that may call them. Until then, a medium whose machine is gone or whose device key was made anew stays closed;
  - the key of a medium that is gone stays kept until deleted.

Evidence, `tests/blockstore_host.rs` (a keeper in memory):
- `a_kept_volume_key_is_nowhere_on_the_medium`: the key in no 32 bytes of the medium, and the store found again with its keeper;
- `a_medium_whose_keeper_has_not_its_key_is_not_mounted`: no keeper, or one without the id, `kept`; another key under it, `key`; nothing written;
- `only_a_random_key_is_kept_and_only_by_a_keeper_that_keeps_it`: a derived key and a refused one stay in the clear;
- `a_kept_superblock_with_bytes_where_the_key_would_be_is_refused`.

On the platform, the QEMU `tls` suite (x86 and aarch64), on a raw image with a blank store disk beside it:
- **its device key check** (308-KRN-0183): the store's volume key is kept as `volume-…`, read back at the next boot, and is not on the medium. Another program's secrets client finds none of it, nor opens the store's file copied under its own name. Under a new device key it opens no more;
- **its TPM check** (swtpm): the disk is made at a boot without a TPM, and an object is put and named. The disk mounts, and the object reads back, after TPM A sealed the device key, and again under A. Under TPM B's new device key it is not mounted (`NOT MOUNTED: ITS VOLUME KEY IS KEPT BY KEYSTORE AS volume-…`), and the object is not read (`key`).

These are tests of the configurations named, not a proof (MC-12.2). The device key on the boot volume beside the disk, without a TPM, is not a protection, as said above.

## The recovery key — host-tested; its calls on the wire, refused to every client so far (308-STO-0028)

**No program can make or use a recovery key yet.** The store's own part, in `blockstore/src/store.rs` and `libmind/src/blockstore.rs`, is tested on the host. Its calls are on the wire (`blockstore.wit` 2.4, below), and answer only a client badged `BADGE_RECOVER`. No program holds one:
- `init` is to mint it and grant it to the shell alone, which lends it on to a program that asks for it (308-KRN-0204, the kernel session's);
- the program that shows a key once and takes it back is `recovery`, with a window of its own, so the key reaches no serial line (308-APP-0122, the tools track's).

Until then, the previous section's limits stand on the platform.

A medium whose volume key `keystore` keeps opens on no other machine, nor on its own once its device key is made anew. A recovery key, kept by the owner away from the machine, is the way back.

- **The key** is 16 random bytes. Without a random source none is made (`no-random`). Only a volume key kept off the medium gets one; a store whose key is in the superblock is refused (`invalid`).
- **Its text form** (`mind::blockstore::recovery_text` and `recovery_parse`) is 32 characters of Crockford's base32 in 8 groups of 4, joined by `-`, for example `000G-40R4-0M30-E209-185G-R38E-1YZ4-BJS6`. They encode the 16 bytes and then the first 4 bytes of their SHA-256, a check. Typing it back ignores case, `-` and spaces, and reads `I` and `L` as 1 and `O` as 0. A `U` or any other character, a wrong length and a failed check are refused as typos, before any key is tried, so a typo is never mistaken for a wrong key.
- **A record in the log, not the superblock.** The store never writes over a stored sector, and a torn write of sector 0 would lose the whole medium. So the volume key sealed under the recovery key is a record like the others, written into blank sectors:
  - its frame's domain is 0xFFFE;
  - inside its seal: `MIND-RCV`, the layout version (u16), six zeros, the volume key (32 bytes), and the SHA-256 of these 48 bytes;
  - it is sealed as every record is (Sealed records above), under the keys derived from K, where K is HKDF-SHA-256 (RFC 5869) of the recovery key with the superblock's kept id as salt and `MIND-STO recovery` as info;
  - the recovery key itself is nowhere on the medium, and neither is the volume key.
- **A mount** cannot open such a record. It keeps it, counts it neither corrupt nor damaged, and remembers where the first 4 lie. A collection keeps it, and a scrub neither opens nor reports it.
- **A new key takes two steps,** so the owner never holds only a key that no longer opens the medium:
  1. `recovery_new` writes the new record, flushes it and returns the key to be shown. Nothing is erased: the older keys still open the medium;
  2. `recovery_keep` takes the key the owner types back. If it opens none of the recovery records, it is `key` and nothing is erased. Otherwise every record it does not open is erased as a freed record is erased (a free marker first, which a mount finishes after a cut).

  So a cut leaves a key the owner holds opening the medium: the old one, both, or the new one once it was shown. The store mounts whole after it. Until the second step, each new key adds a record. Past the 4 places a mount remembers, the others are found by a scan of the frames. **Forgetting** erases every one.
- **On another machine,** `recover` reads the superblock, opens the first recovery record the key opens, checks that the volume key in it opens the superblock's check, and gives it to that machine's keeper under the kept id. The medium then mounts there as its own. Nothing is written to the medium:
  - a wrong key opens nothing (`key`), and nothing is kept;
  - a keeper that refuses the key gives `kept`;
  - a store whose key is in the clear has nothing to recover (`invalid`).
- **The layout stays 4.** A medium without a recovery record is one this build wrote before, so nothing converts anything.

**The wire** (`blockstore.wit` 2.4, a minor version: no earlier call changes):
- **Who may.** `recovery-new`, `recovery-keep`, `recovery-forget` and `recover` need `BADGE_RECOVER` (8, `mind::blockstore`), a right of its own: a recovery key opens the whole medium on any machine, so no other right gives it. They are refused through a memory domain's client, as recovery keys are the volume's. `recoveries`, the count of records, is any client's. Each refusal is logged with the badge (`REFUSED RecoveryNew FOR PID <pid> (BADGE <b>)`).
- **`recovery-new`** writes and flushes a new record and returns the key in its text form, 39 characters. The key goes in the reply alone: no line logs it, and the service zeroes its copies once it has answered. The log says `RECOVERY KEY MADE BY PID <pid>: <n> RECORDS UNTIL IT IS TYPED BACK`.
- **`recovery-keep(key)`** parses the typed text first: a typo is `invalid`, before any record is tried. Then it erases every record the key does not open, and returns how many (`RECOVERY KEY KEPT BY PID <pid>: <e> OTHER RECORDS ERASED, <n> LEFT`). A key that opens none is `key`, and nothing is erased.
- **`recovery-forget`** erases them all and returns how many (`RECOVERY KEYS FORGOTTEN BY PID <pid>: <n> RECORDS ERASED`).
- **`recover(key)`** is for a medium that is not mounted because no keeper of this machine keeps its volume key (`NOT MOUNTED: ITS VOLUME KEY IS KEPT BY KEYSTORE AS volume-…`):
  - the service opens the medium again, gives `keystore` the volume key from the first record the key opens, and mounts the store, which then serves every client as if it had mounted at boot (`RECOVERED BY PID <pid>: …, AND MOUNTED`, then the ready line);
  - a store already mounted, a typo or a key in the clear is `invalid`. A store not mounted for another reason (a damaged medium, another layout) answers that reason;
  - a key that opens nothing, or no secrets client to give the key to, is `key`, and nothing is kept (`NOT RECOVERED FOR PID <pid>: Key`). Without `keystore`'s client the volume key could only go into the superblock in the clear, so it is not recovered.

  The request's copy of a typed key is zeroed once it is answered.
- **`blocks recovery`** prints `RECOVERY RECORDS <n>`. **`blocks recovery try`** asks each of the four calls, with a well-formed key that opens nothing, and prints what the store answers. `blocks` shows no key: the console reaches the serial line, and the shell lends `blocks` its own client, which may not recover.

Evidence, `tests/blockstore_host.rs`: host tests of the logic on a simulated medium with the stand-in cipher, not a proof (MC-12.2).
- `a_recovery_key_opens_a_medium_whose_keeper_lacks_its_key`: a fresh keeper gives `kept`; the recovery key puts the volume key in it, writing nothing; the medium then mounts whole, and the object and its name read back. The record is opened as this section describes it, with HKDF checked against RFC 5869's first test case;
- `a_wrong_recovery_key_opens_nothing_and_writes_nothing`: a wrong key, `key`; a forged record that opens but holds another volume key, `key`; a refusing keeper, `kept`; nothing written or kept; a key in the clear, `invalid`;
- `a_recovery_key_is_written_down_in_groups_and_typed_back_with_a_check`: two texts worked out from the alphabet and SHA-256, not from the code; a round trip of 256 keys; the aliases; every character changed to every other one refused by the check; wrong lengths and characters refused;
- `a_new_recovery_key_replaces_the_old_once_it_is_typed_back`: after a new key, both keys open the medium and there are 2 records; a key typed back wrong is `key` and the medium is unchanged; once the new key is typed back, the first opens nothing, and one record is left, also after a remount;
- `a_rotation_cut_anywhere_leaves_the_owner_a_key_that_opens_the_medium`: a new key and its typing back, on a device that stops after 0, 1, 2… writes and flushes, until a rotation ends before the stop. The old key alone opens the medium, then both, then the new one alone, and a key the owner holds always opens it; each time the medium mounts whole with its keeper;
- `a_recovery_record_is_sealed_kept_by_a_collection_and_not_reported_by_a_scrub`: neither key in any 16 or 32 bytes of the medium; a mount counts nothing corrupt or damaged, a whole scrub reports nothing, and a collection that frees a block keeps the record;
- `a_recovery_key_needs_a_kept_key_and_a_random_source_and_is_forgotten_whole`: a key in the clear, no random source and a read-only medium refused with nothing written; two records forgotten, after which neither key opens anything;
- `recovery_records_past_those_a_mount_remembers_are_found_and_erased`: 6 records of keys never typed back, all erased by a forget, and all but its own by the last key typed back, whose record the mount did not remember;
- `a_domains_records_before_its_key_record_are_found`: a mount that scans twice counts a recovery record once;
- `rights_come_from_the_badge`: the four calls only with `BADGE_RECOVER`, which grants nothing else, and no other right gives them; the count with any right.

On the platform, the QEMU `store` suite (x86 and aarch64), `store_recovery_check`: through the shell's client (badge 7) and the read-only one (badge 1), each of the four calls is refused (`Rights`) and logged with its badge, and nothing is written to the medium. `blocks recovery` gives 0 records on the RAM disk, whose key is in its superblock. Making a key, keeping it and recovering a medium with it have not run on the platform: no program holds a client that may.

**What erasing a recovery record does not reach:**
- a flash medium's spare blocks, which may keep the erased sector beneath its interface, as for a forgotten domain's key record;
- a record whose frame is damaged: it is no longer taken for a recovery record (a damaged magic, for one, leaves its sector counted as damaged), so neither a replacement nor a forget erases it.

**Not provided:** a client that may recover, and a program that shows a key and takes it back (above), so no recovery has run on the platform; the passphrase where there is no TPM (308-STO-0028's other half, with `keystore`'s part in 308-NET-0055 and the boot's wait in 308-KRN-0205).

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
  - a whole store rebuilt from its copy on a second medium (MC-4.8): the names that had a copy are brought back one by one, `blocks rebuild` (309-STO-0041, above), and nothing else of the store;
  - recovery of the store from another store.
- **Releases kept for self-update** ([351-STO-0006](../../issues-done/351-STO-0006-releases-pinned-in-the-store.done), [351-UPD-0033](../../issues-done/351-UPD-0033-the-updater-keeps-its-releases-in-the-store.done)): the boot slots stay on the boot volume. The store holds a copy that a rollback writes a damaged slot again from ([docs/update/updater.md](../update/updater.md#releases-kept-in-the-block-store-351-upd-0033)), not the only source a boot needs.

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

**Accounts and quotas (303-STO-0002, MC-4.11).** What a name's version or a pin retains is charged to an owner: the badge of the client that published or pinned it. The account is the bytes of the distinct roots the owner retains, through the kept versions of names and through its pins, each root counted once however many names and pins hold it. A root's bytes are its object's size (a directory's: the bytes of the files under it); blocks shared between different objects are counted in each. A publication or a pin that would take the account past the owner's quota is refused with `quota`, and nothing is written. `usage` reports the caller's account, quota, names and pins; `pins` lists the first 32 of its pins. The store holds 64 pins in all (the service's static table); a pin past them is refused with `full`.

The quota is fixed: three quarters of the medium for every owner. Nobody grants it yet.

**A reserve for the updater (351-STO-0043, MC-9.3).** On a medium that outlives a reset, a quarter is kept for the updater's owner (`OWNER_UPDATER`), whose pins hold the running, the last known good and a staged release.
- **Every other owner together** retains at most three quarters, each root counted once across them all. A publication, a commit or a pin past that is refused with `quota`, with nothing written, even when the owner is within its own quota.
- **`usage`** gives such an owner, as its quota, what the others together leave it.
- **The updater's own quota** stays three quarters, so it may use free room beyond its quarter.
- **On the RAM disk** nothing is set aside. The store says which at mount (`[BLOCKSTORE] A QUARTER OF THE MEDIUM KEPT FOR THE UPDATER'S RELEASES: …`).
- **What the reserve does not cover:** blocks put and not retained yet, and the records' own room. The reserve bounds accounts, not sectors.

**Owners apart from rights (351-STO-0006).** The owner is the whole badge, below the domain clients' badges (`FIRST_DOMAIN_CLIENT`). Its bits 4 to 7 (`mind::blockstore::OWNER_BITS`) grant nothing, and bits 0 to 3 are the rights. So two clients with the same rights and other owner bits keep their names' charges, their pins and their accounts apart, and neither ends the other's pins.
- `OWNER_UPDATER` (`0x10`) is the updater's owner. With get, put and publish its badge is `0x17`, beside the shell's `7`.
- `init` gives that client to the updater ([351-KRN-0218](../../issues-done/351-KRN-0218-the-updaters-store-client.done)), which keeps its releases as trees pinned by it, and names none ([351-UPD-0033](../../issues-done/351-UPD-0033-the-updater-keeps-its-releases-in-the-store.done)).
- Its account counts each release whole, though the blocks two releases share are stored once. The running, the last known good and a staged release are three, so the store's medium must be at least four times a release for the quota to hold them.

A collection:
- **Mark.** It walks every retained object (`tree::walk`, a file with `dag::walk`, a journal entry after entry): each node is read, checked against its CID and the shape, and marked, and each chunk is marked. If an object lacks a block or holds a corrupt node, the collection refuses (`incomplete`, `corrupt`) and frees nothing. A block the object still needs is never freed because another went missing.
- **Sweep.** One pass over the medium frees what is not marked or leased. Each record is freed in a crash-safe order:
  1. a one-sector marker over its frame (`MIND-DEL`, the sectors it covers, a SHA-256), sealed like any record, flushed;
  2. zeros over its other sectors;
  3. zeros over the marker, flushed.

  A mount that finds a marker finishes the free. A stop in the middle never leaves a false report of damage.

  A block leaves the index before its record is freed (175-STO-0012, audit A05). So a sweep cut by a read, write or flush failure leaves the running store with no erased block indexed: a later put of the same bytes writes them again rather than acknowledging what is gone. Evidence: `tests/blockstore_host.rs` fails each read and each write of a collection in turn and puts every block again (host only: nothing makes the RAM disk fail in the middle of a collection on the platform).
- **Room.** The freed sectors are blank, and later records go into them.

**Pin record (layout 4, sealed).** One sector: `MIND-PIN`, layout version (u16), zero, the id (u32), the owner (u16), zero, the root's CID (36 bytes), and the SHA-256 of these 56 bytes. When mounting, every valid pin record is a pin; a pin's id is never given again while its record exists, and ids go on from the highest found.

Not provided yet:
- **Terms:** a pin lasts until its owner ends it; a pin that ends at a time of its own is not provided (B.3 allows a term or a termination condition; this is the termination condition).
- **Quotas granted per owner** by whoever grants the clients, and a bound on what all owners retain together.
- **Owners apart from badges:** clients that share a badge share an account. Owner bits give two clients of the same rights two accounts (above), but only clients `init` badges apart.
- **Obligations to consumers.**
- **Leases across a restart:** they start again when the store mounts.

Evidence: `tests/blockstore_host.rs`:
- leases and collection, a put that renews its lease, room reused on a full medium;
- nothing freed while a retained object is incomplete, a stopped collection finished by a mount;
- history: an earlier version retains its object until it falls out of the newest 4 (`a_collection_frees_what_no_name_retains_once_its_lease_ends`);
- removal: the name retains nothing, its objects go at the next collection, the name comes back from the removal's version (`a_removed_name_retains_nothing_and_keeps_its_version`);
- pins: an object kept until its owner unpins it, another owner refused, pins found again after a mount (`a_pin_retains_its_object_until_its_owner_unpins_it`);
- two releases as trees, pinned by the updater's owner, kept whole while the shell's owner, with the same rights, names one, points the name at the other and removes it, and cannot end the pins (`the_updaters_releases_outlive_another_owners_names_and_a_collection`):
  - a collection keeps both, every file read back through `tree::files`;
  - once the updater ends the older release's pin, what only that release held goes, and the loader both hold stays;
  - after a mount the updater's one pin is found, its release whole;
  - two deliberate changes of the store, one at a time on a copy outside git, are each caught by this test: an unpin by any owner, and a collection that does not mark pinned objects. This shows the test can fail, not that it finds every fault;
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
| `BADGE_PUBLISH` (4) | `publish`, `unpublish`, `pin`, `unpin`, `copy` (2.5, never through a domain's client) |
| any of them | `stat`, `copy-stat`, and `pins` and `usage` of the client's own owner |

- A client with neither bit may do nothing, and bits this version does not know grant nothing, an owner's bits among them (`OWNER_BITS`, Retention above). A refusal is answered `rights` and logged with the caller's PID and badge.
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
| `copied` | a name with a copy on a second medium, whose change the copy took | the store's medium and the copy's each flushed it (Copies on a second medium, 309-STO-0040); host-tested, and run in QEMU on x86 and aarch64, where `init` lends the copy's disk |

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
| [309](../../issues-done/309-copies-scrub-and-repair.done) | Copies on a second medium, scrub and repair | the scrub built ([309-STO-0025](../../issues-done/309-STO-0025-scrub.done)), copies in a file with repair from them ([309-STO-0031](../../issues-done/309-STO-0031-a-copy-of-named-roots-in-a-file.done)), and copies of chosen names on a second medium with repair from either, host-tested and run in QEMU on x86 and aarch64 (309-STO-0040, above); on real machines after 307 |
| [310](../../issues/310-the-index-on-the-medium.md) | The index on the medium, 64-bit sectors (a layout after 3), longer histories | when sizes ask for it |
| [311](../../issues/311-storefs-a-file-face.md) | `storefs`: a file view of a named root, with snapshots and history | after 307 |
| [312](../../issues/312-the-minds-memory.md) | A journal, assertions and derived indexes, forgetting by domain | the journal built ([312-STO-0034](../../issues-done/312-STO-0034-a-journal-checked-from-its-head.done), above), a text index derived from it ([312-STO-0035](../../issues-done/312-STO-0035-a-text-index-derived-from-a-journal.done), above), and assertions about objects kept in a journal ([312-STO-0038](../../issues-done/312-STO-0038-assertions-about-objects.done), above); no personal data before 308 |
| [313](../../issues/313-persistent-actors.md) | Checkpoints of services with queues, an effect journal | after 308 |
| [314](../../issues/314-models-mapped-from-the-store.md) | Models mapped read-only from the store | after 307 |
| [315](../../issues/315-memory-across-machines.md) | Memory across machines | an intention only |

