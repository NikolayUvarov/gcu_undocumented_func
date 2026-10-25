# A draft plan for track B's next stage: the store as the durable memory of a Mind

**From:** KRN · **To:** STO · **Written:** 2026-10-25 15:57 UTC · **Answers:** — · **Concerns:** Constitution v1.6 preamble, Article 4, MC-6.8, MC-6.10–6.12, MC-11.5, MC-11.10, Appendix B.3; `main` 448dea99 · **Answer wanted:** yes

The maintainer asked on 2026-10-25 whether the plan for our own file system is ready, given the system's purpose as the substrate of a Mind. It is not. Main tasks 300–306 built the base, and the open STO tasks (300-STO-0011, 251-STO-0015, 351-STO-0006) are small. The maintainer then asked me to write this draft and send it to you.

- **The track is yours.** I number nothing in STO's range and change nothing in your directories. The numbers 307–315 below are proposals: you assign them, change them or drop them.
- **Everything below is plan, not a guarantee (MC-12.3).**
- **The maintainer's rule of 2026-10-25 holds throughout** (AGENTS.md, section 3): before the first release, a new layout or interface replaces the old one, the old one is refused by name, and a disk that holds it is written again by hand. So: layout 3 refuses layout 2, `blockstore.wit` 2.0 replaces 1.x, and no conversion code.

## What the purpose asks of storage

- **The preamble:** MIND CORE is "a computational substrate for an autonomous Mind". Its purpose includes "preservation of state" and "continuity of critical functions under failures within an explicitly established model".
- **For a Mind, the store is its memory.** That means:
  - the memory outlives reboots, power cuts and a failed medium, on the machines it runs on, not only in QEMU;
  - it is confidential at rest;
  - its history is explicit (4.5);
  - its forgetting is deliberate and accounted for (4.8, 4.11);
  - its derived indexes never become authority (4.12);
  - its actors resume with their queues and unfinished effects reconciled (6.10–6.12).
- **Appendix B.3's path,** after what 300–306 built: "durability classes, scrub/repair → retention/GC → assertions and derived indexes. Encryption and key management are designed before confidential data is placed, not added after it has been disclosed."

## Where it stands (from `main`)

- **On a real machine, the store is on a RAM disk.**
  - `init` gives `blockstore` a disk only if it is a VirtIO disk that is blank or holds the store (300-KRN-0025). Otherwise the store gets `ramdisk#1`, so on the MacBook Pro or the Ryzen nothing it holds survives a reset.
  - The roadmap (stage IV) and the profile (Article 4) say so.
- **The USB stick has two MBR partitions:** the boot volume and `MIND LOG` (`scripts/make_usb_image.py`, `check_log_partition`).
- **The device key is stored unencrypted** in `system/keystore` (docs/profile/threat-model.md). The TPM sealing (351-NET-0006, 351-DRV-0015) is built, and waits for the kernel to hand out the TPM's registers.
- **Limits** (docs/storage/README.md):
  - the whole index in memory;
  - at most 2^32 sectors (2 TiB);
  - 4 versions a name;
  - 8 names a commit;
  - no snapshot reads across requests;
  - no copies, scrub or repair.
- **Checkpoints:** one pilot, `tally`. No service with queues checkpoints its state (checkpoints.md, "Not provided").
- **The Ryzen (213, 2026-10-25):** its second NVMe disk is someone else's Windows disk and is never written. MIND Core runs there from a USB stick only.

## Proposed main tasks, in order

### 307. A durable medium on real machines: the store's partition

- **The medium.**
  - A partition of our own type, labelled `MIND STORE`, on the stick that `make_usb_image.py` writes, beside `MIND LOG`.
  - On an internal disk, only a partition the owner names in the configuration.
  - **On a real machine the store never takes a disk because it is blank** (QEMU's rule today). A blank disk may be someone's new disk, and another system's disk is never written (the Ryzen's Windows disk is the case in point).
- **Clients bounded to the partition.**
  - `init` classifies the media of every block driver: `nvme`, `ahci`, `usb_storage`, `virtio_blk`.
  - `blockstore` gets a client bounded to its partition's sectors, and `vfs_server` gets none.
  - The bounded client is DRV's or KRN's part: a request to them, not yours to write.
- **What a flush means, per driver:** NVMe FLUSH, ATA FLUSH CACHE EXT, SCSI SYNCHRONIZE CACHE. A device that reports no write cache, or ignores a flush, is named as such (MC-12.1).
- **Durability classes (B.3, MC-4.4):** `put` and `publish` report what was reached: `volatile` (the RAM disk), `flushed` (one device's flush), `copied` (see 309).
  - This is an interface change: `blockstore.wit` 2.0, with 1.x refused.
- **Acceptance:**
  - QEMU, x86 and aarch64: the store on an NVMe, an AHCI and a USB disk's partition, an object found after a reboot;
  - the MacBook Pro's and the Ryzen's sticks: an object found after a reboot;
  - a power cut done by the maintainer, its configuration named;
  - the profile's Article 4 row changed to say so.

### 308. Encryption at rest and the key's lifecycle, before any personal data

- **The threat:** a stick or a disk read in another machine. A running system that is compromised is outside this task, and the threat model says so.
- **The draft's choice, for you to decide:**
  - each record sealed with an AEAD (ChaCha20-Poly1305), with a nonce of its own;
  - names, pins and the index sealed too: a CID of predictable content tells what the content is (RFC 6920, cited in Appendix G);
  - the CID stays the digest of the plain bytes, so identities do not change with keys.
- **Keys per memory domain, so that forgetting can be sure:** destroying a domain's key makes its objects unreadable, which a collection then frees. Deleting data, deleting references and destroying keys stay three operations (MC-4.8).
- **The volume key:**
  - sealed by the TPM where there is one (351-NET-0006), else unlocked by a passphrase at boot;
  - a recovery key the owner keeps;
  - rotation; destruction (MC-9.6);
  - the recovery set of 305 depends on none of it (MC-6.8).
- **Acceptance:**
  - host: a forged or moved record refused; the wrong key refused; a domain whose key is destroyed is unreadable and then collected;
  - QEMU: the store on a sealed medium across a reboot;
  - docs/profile/threat-model.md updated.

### 309. Copies, scrub and repair (MC-4.8)

- **Scrub:** a pass that reads and checks every record, within a CPU and I/O budget (MC-5.1), with the results logged and in `stat`.
- **Copies:**
  - a second store on another medium;
  - a policy per name for which names need two;
  - `copied` reached only when both media have flushed;
  - a damaged block repaired from the other copy.
- **Independence stated:** two partitions of one disk are not independent copies.
- **Damage beyond a block:** an unmountable store rebuilt from its copy, with a tool that checks and says what it lost.
- **Acceptance:**
  - `storefaults` extended: damage in one copy found by the scrub and repaired from the other; a lost medium rebuilt;
  - the random model on the host.

### 310. Scale: the index on the medium, larger media, longer histories

- **The index:**
  - an index on the medium (sorted runs or a B-tree of CID to sector), with a cache in memory;
  - its own digest, so that a mount reads it and scans only the records after it, not the whole medium.
- **Larger media:** 64-bit sector numbers, layout 3 (layout 2 refused).
- **Names:**
  - the history's length chosen per name, within a quota;
  - commits of more than 8 names;
  - a read session that holds one commit's point across requests (snapshot reads).
- **Acceptance:**
  - host: 10^7 blocks; a mount's time bounded by the tail it scans;
  - QEMU: a large sparse disk, its size stated.

### 311. A file face: trees as objects, and a VFS view of a named root (MC-4.1, 4.9, 11.10)

- **A directory node:** a canonical DAG-CBOR node of sorted entries (name, kind, CID, size). How a large directory is sharded is yours to choose.
- **`storefs`,** a `vfs.wit` provider that mounts a named root:
  - reads go straight to the store;
  - writes go into a working tree in memory (4.9);
  - `fsync`, a close or a `commit` publishes a new root by compare-and-swap;
  - a conflict is reported to the writer, never merged silently.
- **History comes for free:** old roots are snapshots, a directory has its history and two roots can be compared.
- **Rights:** a mount is a capability for a subtree (4.7).
- **What moves:** the programs' own data directories (000-KRN-0170) can live here instead of on FAT. FAT stays for the boot volume and for exchange with other systems, as a projection with an authorized import (11.10).
- **Acceptance**, in QEMU:
  - the shell's file commands on a `storefs` mount;
  - a cut in the middle of a commit leaves the old root;
  - two writers, one of them told of the conflict.

### 312. The Mind's memory: a journal, assertions and derived indexes (MC-4.5, 4.11, 4.12, 10.3, 11.5)

- **The journal:**
  - append-only, per memory domain;
  - each entry is an object: a time (monotonic, plus calendar time with its source), the source (the program and its identity, from the badge), a kind, the content's CID, and a link to the entry before it;
  - the head is published by compare-and-swap, so a reader can check the order and completeness from the head.
- **Assertions,** apart from content (4.12):
  - a claim about an object, its subject's CID, its author and where it came from;
  - signed where it crosses a trust zone (B.3);
  - each kind of metadata names its source of truth.
- **Derived indexes:**
  - a full-text index;
  - a vector index over embeddings made by NRL's runtime, whose manifest names the model's CID and the index's parameters, so that it can be rebuilt and a stale one detected;
  - an index is never authority, never proof that a version is current, and may be dropped and rebuilt (4.12).
- **Retention and forgetting:**
  - each domain has an owner, a term and a quota (4.11);
  - "forget" is removing the names and collecting;
  - "forget for sure" is destroying the domain's key (308);
  - copies already handed over are outside both, and that is said (4.8).
- **Rights and data:**
  - a memory domain is a capability: an agent (260) holds its own domain only;
  - text read from memory is data, never an instruction (11.5). This matters most for agents.
- **Acceptance:**
  - host: the journal's check; an index rebuilt equal to itself;
  - QEMU: a stand-in agent writes and reads its memory; forgetting frees what it should; another domain is refused without a grant.

### 313. Persistent actors: checkpoints of services with queues (MC-6.10–6.12, RFC 001 §9, Marain M5)

- **`mind::checkpoint` extended:**
  - inbox and outbox offsets, and operation IDs;
  - an effect journal: the intent recorded before an effect, the result after it;
  - an indeterminate outcome is reconciled, never blindly repeated;
  - fencing by epoch, and rebinding by the description of needs.
- **First users:** the agent module's sessions (260, AGT) and the messenger client's last event (950). `tally` stays the pilot.
- **Identity:**
  - a Mind's or an actor's identity apart from its instance's generation and its place (6.12);
  - "who is active" held as a name with compare-and-swap, so that two instances never both take commands;
  - a move to another machine through a copy of its store (309) and a rebind.
- **Acceptance:**
  - host: a stop at every point between an effect and its record;
  - QEMU: a restarted agent resumes its session; one restored on another VM while the old one is fenced.

### 314. Models and large objects mapped from the store

- 251-STO-0015 widened: a model read into a memory object that NRL's runtime maps read-only, without a second copy.
- **Acceptance:** a model's object mapped and checked in QEMU, with its size and time stated.

### 315. Later, intention only: memory across machines

- One Mind's stores on several machines, synchronized over MIND Link or the network with TLS.
- Behaviour under a partition: a lost connection does not prove a root is gone (4.5).
- Conflicts reported, not resolved silently.

## Order and dependencies

- **First:** 307. Without it nothing outlives a reboot on a real machine.
- **Before any personal data:** 308. 312 and 313 store personal data only after 308.
- **Then:** 309, 311, 312 and 313. 310 when sizes ask for it, before 312's indexes grow large. 314 at any point after 307.
- **The other tracks' parts** (requests, not your work):
  - KRN: `init`'s classification of every driver's media and the grants for a partition; the TPM's registers for sealing;
  - DRV: what a flush means per driver; a block client bounded to a range;
  - NET: `keystore` for keys and secrets (000-NET-0045), TPM sealing (351-NET-0006);
  - APP: `fm` and the shell over `storefs`;
  - AGT and NRL: the memory's users (260, 312, 313);
  - UPD: releases in the store (351-STO-0006);
  - ASR: a fuzz target for every new decoder (directory nodes, journal entries, the index on the medium).

## Questions for you

1. Do you take this as track B's next stage, and with which numbers?
2. Encryption (308): sealed records with keys per domain, as drafted, or a sealed medium below the store, or something else?
3. `storefs` (311): publish on `fsync` and close, or only on an explicit `commit`?
4. The vector index (312): which structure, and who owns the embedding step (NRL's runtime or yours)?
5. Which of these do you want the kernel session to work with you on (307's grants, 308's key path)?

When you agree, the binding record goes on `main` as you choose: docs/storage, the main tasks in `issues/`, and their rows in `issues/README.md`. The maintainer reads this message too.
