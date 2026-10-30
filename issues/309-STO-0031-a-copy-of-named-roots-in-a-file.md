# 309-STO-0031 — A copy of named roots in a file: export, import, and repair from it

**Type:** storage (`libmind` `car`, `blocks`) · **Owner:** `STO` track · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [309](309-copies-scrub-and-repair.md) · **Constitution:** MC-4.4, MC-4.8, MC-6.8

Split from [309](309-copies-scrub-and-repair.md) (2026-10-30), its second step, after the scrub. Unlike 309's live second store, it needs no second medium lent to the store, so it waits for no other track.

## Problem

The store keeps one copy on one medium. A damaged block is found by a read or a scrub ([309-STO-0025](../issues-done/309-STO-0025-scrub.done)), but there is nothing to repair it from. A lost medium, or a RAM disk after a reset, loses every name. And an instance cannot be moved to another machine ([313](313-persistent-actors.md)), as nothing carries its blocks and names there.

## Plan

- **The format is CAR v1** (the IPLD content archive), so a copy is readable by other tools:
  - **a header** (a varint length, then DAG-CBOR `{"roots": [cid], "version": 1}`). Its one root is a names node;
  - **sections**, each a varint length, then a CID, then its block.
- **The names node:** canonical DAG-CBOR `{"v": 1, "names": [[name, version, root], ...]}`, sorted by name, each name's root linked. It is decoded only if it is exactly that encoding, as `tree` does.
- **`mind::car` (`libmind/src/car.rs`):** no heap and no system calls.
  - A writer of sections and a reader of them.
  - Every block read is checked against its CID. A section cut short, a length past a block, a CID of another hash or codec, and a header that is not exactly its encoding are refused.
- **`blocks export <file> <name>...`:**
  - the names' current roots, read at one point (`snapshot`);
  - the names node, then every block under each root (`tree::walk`). A block met twice may be written twice; an import puts it once;
  - it prints the number of blocks and bytes, and the SHA-256 of the file.
- **`blocks import <file>`:**
  - checks every section first, and refuses the whole file on one bad section, with nothing put;
  - then puts each block. A block the store holds damaged is repaired by the put, as a put of the same bytes repairs it now;
  - then publishes each name at the file's root by compare-and-swap, from the version it is at in this store (0 for a new name). It prints each name published, or a name whose current root differs, which it leaves as it is: a conflict, never overwritten.
- **`blocks import --check <file>`** reads and checks a file, and puts nothing.
- **Independence stated:** a file on the store's own medium is not an independent copy; docs/storage says what is.

## Decisions while building (2026-10-30)

- **Built in a worktree of the storage session,** as 311-STO-0027 and 0029 were; for the session to review and merge.
- **A copy is checked by walking it.** Each name's root is walked (`tree::walk`) over the copy's own sections, and each block the walk visits must be the next section. A copy cut at a section's end is still a CAR v1, of fewer blocks; checking each section alone would take it. The walk refuses it (`missing`), and so a copy missing a block, one with a block out of the walk's order (`order`), and a section after the last. So every truncation is refused, as the criteria ask. A copy is taken only in the order an export writes it: a CAR another tool wrote in another order is refused.
- **A walk reads nodes again:** a directory after each entry, an object's root for each node of height 1. The check finds them among the sections met: the last one, the last 32 `dag-cbor` ones, the next one (a walk reads its root before it visits it), else by reading the sections again from the first. That last way costs a pass over the sections met; it is taken when a directory's subtrees hold more than 32 nodes. A host test with 40 takes it, and fails without it.
- **An import reads each block from the store before it puts it.** A scrub only reports: the store's index still names a damaged block the scrub found, so a put of the same bytes would find it held and write nothing. The import's read finds it corrupt and takes it out of the index, as any read does, and the put then stores it again. After a repair the import asks for a collection, which frees the damaged copy, so the next scrub finds nothing. A block a mount found damaged has left the index already: it is stored as one the store lacked, and the next collection frees its damaged copy. Nothing in `blockstore` or its interface changed.
- **The file is read twice:** once for the check, once for the puts. The second read must give the same bytes and SHA-256 (`changed`), or nothing is published. A section that changed in between is refused where it is met, and the blocks put before it are retained by nothing until a collection.
- **Names:** at most 8 (`car::NAMES`), as one `snapshot` reads them at one point. A removed name has no root to overwrite: like a new name, it is published from the version it is at, its removal's. A name another writer publishes between the import's read and its publication is read again: at the copy's root it is `same`, at another a conflict. Any other refusal of a publication (for example `incomplete`) stops the import with the store's error, and the names published before it stay.
- **The names node is the copy's alone.** It is a `dag-cbor` block of neither of the store's schemas, so it is not put. Its `version` is the one the name was at where it was exported; an import publishes from this store's version, so into an empty store a name comes back at version 1.
- **`blocks export` makes the file at its first byte:** a refusal before it (a name refused, removed or missing) leaves a file of that name as it was, and a copy that failed part-way is removed. Its walks take the steps the store gives its own: 4 for each block its index can hold, and 64 (`stat`'s capacity).
- **Errors are the copy's own** (`car::Error`): `Truncated`, `Format`, `Version`, `TooLarge`, `Cid`, `Corrupt`, `Block`, `Missing`, `Order`, `Name`, `Changed`, `File`, and `Store` for the store's. `car::Refused` adds the byte the refused section starts at.
- **`mind::car` holds the export and the import,** over a trait `car::Store` (`dag::Blocks`, `worktree::Names`, `snapshot`, `collect`), a `Source` and a `Target`. So the host tests run the logic `blocks` runs, on the store's own logic. `cid::varint` is crate-visible now.
- **`blocks` outputs:** `EXPORT`, `EXPORTED`; `NAME`, `CHECKED`; `PUBLISHED`, `SAME`, `CONFLICT … LEFT AS IT IS`, `IMPORTED … (<s> STORED, <r> REPAIRED, <h> HELD)`, `COLLECTED … THE DAMAGED COPIES FREED` (docs/storage).
- **QEMU `storefaults`:** the check runs after the suite's first repair, not at its end. The damaged frame the suite injects later leaves 33 sectors damaged for good, which every later scrub reports. And the suite ends with a store that is not mounted. The suite writes the expected copy itself from the specifications, and compares its SHA-256 with the export's.
- **QEMU `store`:** the suite's machine may now reboot (`reboot=True` for this suite alone). The boot volume is QEMU's virtual FAT, whose snapshot overlay keeps the guest's writes across the reboot, as `store_disk_check` relies on; the RAM disk store starts blank. The check is the suite's last.
- **Found, not changed:** a checkpoint does not move through a copy yet. Its names come back at new versions, and `checkpoint::restore` refuses a manifest whose epoch disagrees with its names' version (read in checkpoint.rs, not tested here; 313).

## Progress (2026-10-30)

- **Built** in a worktree by an agent of the storage session, for the session to review and merge. On this session's container (x86, TCG, 4 CPUs):
  - `tests/car_host.rs` (9 tests), and `tree`, `worktree`, `dag`, `checkpoint`, `cid`, `blockstore` pass; `scripts/host_tests.sh` whole exits 0;
  - the QEMU `store` and `storefaults` suites (x86) pass, the two new checks among them.
- **The tests can fail:** with `Reader::next`'s check against the CID left out, three host tests fail; with a name at another root published over, the conflict test fails (experiments on copies of `car.rs`).
- **Not done here:** aarch64; the profile (README, evidence), which is the session's to update.

## Acceptance criteria

- Host tests (`tests/car_host.rs`, through the store's logic on a simulated medium):
  - CAR v1 vectors (a header and a section built by hand); every truncation, a byte changed in any section, a CID of another hash or codec, and a non-canonical names node refused;
  - export then import into an empty store gives the same names, roots and blocks;
  - an import into a store holding a damaged block repairs it, and a scrub then finds nothing;
  - a name whose root differs is left as it is and reported; nothing is put from a file with one bad section.
- QEMU `storefaults` (x86, aarch64): a chunk damaged on the medium is found by `blocks scrub`, then repaired by `blocks import` from a copy exported before, and the next scrub finds nothing.
- QEMU `store` (x86, aarch64): names exported to a file on the boot volume come back, whole, into a store that lost them (a RAM disk after a reset, or a blank store disk).
- docs/storage and the profile.

## Related

[309](309-copies-scrub-and-repair.md), [309-STO-0025](../issues-done/309-STO-0025-scrub.done), [311-STO-0027](../issues-done/311-STO-0027-directories-as-objects.done), [313](313-persistent-actors.md) (a move through a copy of its store).
