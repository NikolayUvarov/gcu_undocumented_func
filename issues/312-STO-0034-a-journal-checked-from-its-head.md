# 312-STO-0034 — A journal, append-only, checked from its head

**Type:** storage (`libmind`, `blockstore`, `blocks`) · **Owner:** `STO` track · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [312](312-the-minds-memory.md) · **Constitution:** MC-4.3, MC-4.5, MC-4.10, MC-4.12, MC-10.3

Split from [312](312-the-minds-memory.md) (2026-10-30), its first step: "the journal, append-only per memory domain … the head is published by compare-and-swap, so a reader checks order and completeness from it". Assertions and derived indexes are later tasks of 312.

No personal data is written before [308](308-encryption-at-rest.md) is done: the tests and the pilot write test text only.

## Problem

The store keeps objects and names, and a name's history keeps 4 versions. Nothing keeps an ordered record of what happened that a reader can check whole:
- every entry there;
- none reordered, changed or dropped;
- the record never rewritten from an older point.

## Plan

- **An entry** is a canonical DAG-CBOR node of a schema of its own, `mind::journal`, versioned (`"v"`; another is refused, MC-4.13). It holds:
  - its sequence number, from 1;
  - the time: the monotonic time and the boot it belongs to, and the calendar time with its source (none, the clock chip, the network). The calendar time is the writer's claim, and the check never orders by it;
  - the source, as the writer gives it: a program's name;
  - a kind: a short label;
  - the content's CID, a link;
  - a link to the entry before, absent for the first.
- **The store retains a journal whole.**
  - It takes an entry as a typed `dag-cbor` block, as it takes `dag`'s nodes and `tree`'s directories.
  - Its walk follows the links to the entry before one after another, never nested. `tree` bounds a path at 32 directories, and a journal is longer than that.
  - So a collection keeps every entry and every content a journal's head retains, and `dag::complete` refuses a head whose chain lacks a block.
- **Appending** puts the content and the entry, then publishes the entry under the journal's name by compare-and-swap from the version read.
  - Of two writers that read the same head, the second gets `conflict`, reads the new head and appends after it. It never forks the journal.
  - An entry's sequence number is the name's version it is published at.
- **The check from the head** reads the name's current version and root, then walks the chain to the first entry, within a bound of steps. It refuses:
  - a sequence number that is not the version, the next one down, or ending above 1;
  - a block missing or corrupt, an entry of another schema;
  - a monotonic time that goes back within one boot.

  A head published again at a later version, a rewrite, is refused this way, as a checkpoint's epoch is (313-STO-0033). So is a journal whose name was removed and published again.
- **A memory domain:** a journal in a domain is written through that domain's client (308-STO-0023), and is forgotten with it. Nothing new is needed for this; the tests show it.
- **The pilot, `blocks journal`:**
  - `append <name> <kind> <text>` and `append <name> <kind> -f <file>`;
  - `check <name>`, which prints the count, the first and last sequence numbers, and the result;
  - `list <name> [n]`, the last entries, newest first;
  - with `-d <domain>`, as every command of the tool.
- **Not on the wire:** `blockstore.wit` does not change. A journal is a client's use of `put`, `publish`, `resolve` and `get`.

## What it does not give, said in docs/storage

- **The source and the calendar time are the writer's claims.** The store records the publisher's badge in a name record, but `history` does not return it. A source attested by the store is a later step, on the wire.
- **A writer with the name's rights can append anything.** The check shows the order and completeness of what was published, not that the entries are true (MC-4.12).
- **A journal is as durable as its store** (307-STO-0019's classes). A copy (309-STO-0031) carries it whole, since a copy is of a name's object. But an import publishes the name at the versions of the store it comes into, so the check there refuses the head (see the decisions below): a journal's order is checked in the store it was written in.

## Decisions while building (2026-10-30)

- **The schema** is flat: `{"v": 1, "seq", "boot", "kind", "prev", "clock", "source", "content", "calendar", "monotonic"}`, its keys in DAG-CBOR's order, as the reference library `dag-cbor` 0.3.3 sorts them (a host test holds two of its encodings).
  - The calendar time is seconds since 1970-01-01 UTC, and its clock a number: 0 none, 1 the clock chip, 2 the network. Without a clock the time is 0, so "none" has one encoding.
  - `prev` is absent exactly when `seq` is 1, a rule of the schema. So a chain that ends above 1, or goes on below it, has an entry that is no entry (`format`): neither the store nor the check ever meets one.
  - Kind and source: 1 to 32 bytes of `!` to `~`, as a checkpoint's labels but never empty. The journal has a `Label` of its own, so the store's logic does not take in `checkpoint`.
- **The boot is counted by the journal.** No boot identity exists on the platform: no system call or `BootInfo` field names the boot, and x86's `qemu64` has no RDRAND to make one. An append takes the head's boot, or the next when the head's monotonic time is past the writer's (its clock started again); the first entry's is 1. A new boot whose clock has already passed the head's is so counted as the same one. The check refuses a boot that goes back along the chain, and a monotonic time that goes back within one boot. A boot identity from the kernel would replace the count; it is asked of no track yet. docs/storage says the boot is the writer's claim.
- **`append` takes the content's CID,** the content put first by the writer (a `raw` block, or an object of any size), as `checkpoint::save` takes the state's object stored. Its publication's walk refuses a content not stored (`missing`).
- **An append refuses** a head the check would refuse at once (no entry of this schema, or not at its version) and a removed name (`Removed`), so it never extends a journal that no longer checks.
- **The store's walk is `journal::walk`.** It follows a journal's entries; for any other root it is `tree::walk`. It reads a `dag-cbor` root once more to tell an entry from a node or a directory.
  - It checks each link before against its sequence (one less), as `tree` checks an entry's size: out of shape is `invalid`.
  - It checks neither the times nor the version: they are the reader's. So a journal with a time going back can be published, and the check refuses it, as the acceptance criteria ask.
  - `tree::walk_from` and `size_from` take their steps from a count the caller holds, so one walk's bound covers a journal and all its contents.
- **`dag::complete`** checks one object of `dag`, and only the host tests call it. The store's check that a root is complete, at a publication, a pin or a commit, is its walk. So "`dag::complete` refuses a head whose chain lacks a block" is met by the store's walk refusing it (`incomplete`), not by `dag::complete`.
- **The owners' sizes:** a journal's size is the bytes of its contents' files, as a directory's is its files' (`journal::size`).
- **Copies.** `mind::car` walks with `journal::walk`, so a copy of a journal's name carries every entry and content. An import publishes the name at version 1 of an empty store, and the check refuses it there (`Sequence`). It is not taken as the first version, as a checkpoint's manifest is (313-STO-0033): a head at version 1 of another store is also what a copy of an older head would look like, and the check cannot tell the two apart. docs/storage says so, and a host test shows it.
- **`check` and `list`** share one walk down from the head. A refusal says where it stopped (`Refused { error, version, at, checked }`), and an entry whose bytes do not match its CID is `corrupt`: the store is another process.
- **`blocks journal`:**
  - the text is the words of the command line joined by one space (the shell splits them); the source is `blocks`;
  - the calendar time comes from the `rtc` service in UTC, or there is none;
  - up to 8 conflicts are met by appending again;
  - `list` gives 10 entries by default, on two lines each, as a console line holds 256 bytes;
  - `check` reads within the store's own bound on a walk.
- **The host tests are built optimized,** with debug assertions, in `scripts/host_tests.sh`, as `blockstore`'s are, not in its list of unoptimized ones. Unoptimized they take 61 s, as the 1000 appends each walk the whole chain; optimized, about 6 s.
- **The QEMU check, `store_journal_check`,** runs after `tally_move_check`. It waits for the leases on its own (62 s by the guest's clock), so its collection frees what nothing retains. The suite computes each entry's CID from the schema (`journal_entry`, written from the DAG-CBOR specification) and the times `list` prints. `store_domains_check` also appends to a journal in a domain, where one is made (aarch64).

## Progress (2026-10-31)

- **Built** in a worktree by an agent of the storage session, for the session to review and merge. On the agent's container (an x86 host of 4 CPUs, QEMU under TCG for both architectures):
  - `tests/journal_host.rs`: 16 tests pass, built with `-O -C debug-assertions=on` (6 s) and without (61 s). `tree`, `dag`, `car`, `worktree`, `storefs`, `checkpoint` and `blockstore` pass; `scripts/host_tests.sh` whole exits 0;
  - the QEMU `store` suite (x86) passes whole, `store_journal_check` among it; the calendar time came from the clock chip;
  - the aarch64 image builds (`ARCH=aarch64 ./02_build.sh`), and its QEMU `store` suite passes whole here too, with the journal in a domain (`store_domains_check`).
- **The tests can fail.** Each change below was made to a copy of the code, and the journal's host tests were run on it:
  - the check not comparing the head's sequence with the name's version: 4 fail (an older head, a name removed and published again, an entry changed, a copy imported);
  - the walk not following `prev`: 6 fail (the 1000 entries after a collection, an entry erased, two writers, the walk's steps, the bound on steps, a copy);
  - the check comparing no monotonic times, or boots alone: 1 fails (the time that goes back);
  - an append publishing from the version read again just before, not the one it built on: 2 fail (two writers, the stopped append);
  - the store's walk not checking the sequence of the entry before: 1 fails (the walk's shapes);
  - the check not matching a block against its CID: 2 fail (an entry changed, a store that lies);
  - an append never counting a new boot: 1 fails;
  - a check past its steps passing: 1 fails; an append extending a head not at its version: 1 fails;
  - the store taking no entry: 13 fail; a copy walking with `tree::walk`: 1 fails; decoding without the canonical re-encoding: 1 fails.
- **Not yet:** the session's runs on the maintainer's machines, and the profile's Article 4 rows.

## Acceptance criteria

- **Host,** `tests/journal_host.rs`, over the block store's logic on a simulated medium:
  - 1000 entries appended, checked whole from the head, also after a remount and a collection;
  - the check refuses each of these:
    - an entry's block erased;
    - an entry changed (another entry with the same links);
    - an older head published again at a later version;
    - a name removed and published again;
    - a monotonic time that goes back within one boot;
  - two writers: the second refused with `conflict`, appending after re-reading, and the check passes;
  - a journal in a memory domain, gone with the domain's forgetting;
  - an append stopped after each of the store's calls: the check passes on whatever the head is.
- **QEMU `store` suite (x86, aarch64):**
  - `blocks journal append` three times;
  - `check` and `list` print them;
  - after a collection they are still there;
  - an older head published again by hand with `blocks publish` is refused by `check`.
- **docs/storage** (a section on the journal, with what it does not give) and the profile's Article 4 rows.

## Related

[312](312-the-minds-memory.md), [311-STO-0027](../issues-done/311-STO-0027-directories-as-objects.done), [308-STO-0023](../issues-done/308-STO-0023-memory-domains.done), [313-STO-0033](../issues-done/313-STO-0033-a-component-moved-through-a-copy.done), [303](../issues-done/303-retention-and-collection.done).
