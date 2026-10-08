# Checkpoints: the contract and its protocol

**Version:** 1 (2026-10-08) · **Track:** `STO`, main task [306](../../issues-done/306-checkpoints-and-rebinding.done) · **Constitution:** [v1.6](../../constitution/EN/MIND_CORE_Constitution_v1.6.md) MC-6.10, 6.11, 6.12, Appendix B.4

A component that claims to save and restore its state follows a versioned checkpoint contract (MC-6.10). This document defines the checkpoint format and the protocol of `mind::checkpoint` (`libmind/src/checkpoint.rs`, format version 1), and the contract of its one user so far, the pilot program `tally`. Persistence is not assumed for any other component: no service checkpoints its state. What is implemented and tested is stated here and in the profile; anything else is plan (MC-12.3).

## The checkpoint

A checkpoint of component `c` is two objects in the block store, published under two names by one commit ([304](../../issues-done/304-several-names-at-once.done)):

| Name | Holds |
|---|---|
| `checkpoint/c` | the manifest: one raw block in the format below |
| `checkpoint/c/state` | the state's object, in the component's own schema |

Both names change together or not at all, so the manifest always describes the state its name holds. A restore reads both names in one `snapshot` and refuses a checkpoint whose versions or roots disagree (`inconsistent`).

**The manifest** (format version 1; every integer little-endian, every label one length byte and 1 to 32 printable ASCII bytes):

| Field | Meaning |
|---|---|
| `MIND-CKP`, format version (u16) | another format version is refused, never read as this one |
| contract (label), contract version (u16) | the contract the component follows; a restorer accepts only its own |
| schema (u16) | the state's schema version |
| epoch (u64) | the generation of the instance that saved it: the version of the checkpoint's names it wrote |
| sequence (u64) | the consistency point: how many requests the state includes |
| state (CID, 36 bytes) | the state object's root |
| counts (3 × u8) | resources (at most 8), authorities (at most 8), effects (at most 16) |
| resources (labels) | logical names of what the state refers to: services, names. Never a handle or an address (MC-6.11) |
| authorities (labels) | the rights the instance used, as text. Never a capability |
| effects (id u64, outcome u8, label) | the effect journal: 0 pending, 1 done, 2 failed |

A manifest decodes only if it is exactly one encoding: no bytes after the last field, counts within bounds, valid labels and outcomes.

## The protocol

- **Restore.**
  1. Read both names in one snapshot.
  2. If neither exists, or both are removed at the same version, the component starts fresh at that epoch.
  3. Otherwise read the manifest block. Its CID is checked by the store. Check that its format, contract, contract version and schema are the restorer's, and that its epoch and state root match the names.
  4. Read the state's object (every block checked against its CID) and parse it in the schema.
- **The consistency point** is the commit that saves a checkpoint. A request is applied to the state in memory, the new state's object is put, and the manifest is saved with `sequence + 1`. The change exists only from that commit on.
- **Acknowledgement.** A component answers the sender of a request only after the commit returns. Until then the request may be lost. The permissible loss is every request not acknowledged.
- **Receipts and effects.** The component keeps its own receipts in the effect journal:
  1. Before an external effect begins, the component records it as pending and saves a checkpoint.
  2. After the effect, it records the outcome and saves again.

  If the instance ends between the two saves, the next instance finds the effect pending. Its outcome is unknown, so it must be reconciled with whoever can know it (the effect's target, an operator), and never tried again blindly (Appendix B.4). A component may refuse new effects while one is pending; `tally` does.
- **Fencing (MC-6.12).** The checkpoint names' version is the instance's generation. A save names the epoch it restored from, and the commit is a compare-and-swap on both names. If another instance saved since, the save is refused (`fenced`) and nothing is written. The stale instance has lost the component and must stop accepting requests. Two instances can never both continue from the same checkpoint.
- **Rebinding (MC-6.11).** Restore returns the recorded authorities as text. A restored instance takes its rights from its current grants: what `init` and the shell give it now, never from the checkpoint. `rebind` lists the recorded authorities the instance does not hold now. Protected actions that need them are refused or degraded, and the checkpoint cannot return a revoked right.
- **Retention.** The checkpoint's names keep their last 4 versions, and those retain their manifests and states ([303](../../issues-done/303-retention-and-collection.done)). Recoverability is promised for the last 4 checkpoints, under the store's own durability (on the RAM disk, until the next reset). The code that reads them, `tally.elf`, is on the boot volume. Its schemas are versioned in the manifest.

## The contract of `tally`, version 1

`tally` keeps named counters (`tally add <key> [n]`). Each run is a new instance: it restores, applies one request and saves.

| Item (MC-6.10) | `tally` 1 |
|---|---|
| Composition of state | Counters, at most 32 keys of at most 16 printable bytes. The object is one raw block of lines `key=count`, sorted by key (schema 1) |
| Consistency boundary | One request (`add`, the two saves of an `effect`, `reconcile`); `sequence` counts them |
| Dependencies | The block store. A resource is recorded as `blockstore` |
| Authorities used | `blockstore` (the shell's client) and `files` (for effects) |
| Permissible loss | A request whose save did not return `SAVED`. Nothing after `SAVED` (to the store's durability) |
| External effects | `effect <file>` writes `ram:<file>`. Its intent is saved first; `reconcile <id> done\|failed` settles a pending one, and no new effect begins while one is pending |
| Verification | Restore checks the manifest's format, contract, schema and epoch against the names, and every block against its CID. `tally show` prints the epoch, sequence, counters and pending effects |
| Fencing | A save from a stale epoch prints `FENCED` and writes nothing |

## Evidence

- `tests/checkpoint_host.rs`, through the block store's own logic on a simulated medium:
  - the manifest's one encoding, and its refusals (every truncation, a trailing byte, another magic, format or outcome, counts past bounds);
  - save and restore after a remount;
  - a stale instance fenced;
  - another contract, contract version or schema, a state name moved alone, and a manifest that is not one, each refused;
  - effects coming back pending and reconciled, and a full journal keeping every pending effect;
  - authorities checked against what an instance holds.
- The QEMU `store` suite (x86 and aarch64):
  - `tally` counts across instances, and the checkpoint's two names move together;
  - an instance that restored epoch 3 and stalled is fenced when another saves epoch 4;
  - an effect cut short (its file written, its outcome not recorded) comes back pending, new effects are refused until it is reconciled, and the next effect completes.

These are tests of one pilot, not a proof (MC-12.2). Not provided:
- checkpoints of services (their inboxes, outboxes and unfinished replies: the persistent-execution profile of MC-6.10 for actors with queues);
- migration of an instance to another place;
- a test that a recorded authority the instance lacks is refused on the platform (the shell lends `tally` both rights; the refusal is host-tested).
