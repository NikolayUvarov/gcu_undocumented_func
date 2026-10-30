# Checkpoints: the contract and its protocol

**Version:** 2 (2026-10-30): format 2, operation IDs, the inbox and the outbox, `tally` 2 (313-STO-0026); 1 (2026-10-08) · **Track:** `STO`, main tasks [306](../../issues-done/306-checkpoints-and-rebinding.done) and [313](../../issues/313-persistent-actors.md) · **Constitution:** [v1.6](../../constitution/EN/MIND_CORE_Constitution_v1.6.md) MC-6.10, 6.11, 6.12, Appendix B.4

A component that claims to save and restore its state follows a versioned checkpoint contract (MC-6.10). This document defines the checkpoint format and the protocol of `mind::checkpoint` (`libmind/src/checkpoint.rs`, format version 2), and the contract of its one user so far, the pilot program `tally`. Persistence is not assumed for any other component: no service checkpoints its state. What is implemented and tested is stated here and in the profile; anything else is plan (MC-12.3).

## The checkpoint

A checkpoint of component `c` is two objects in the block store, published under two names by one commit ([304](../../issues-done/304-several-names-at-once.done)):

| Name | Holds |
|---|---|
| `checkpoint/c` | the manifest: one raw block in the format below |
| `checkpoint/c/state` | the state's object, in the component's own schema |

Both names change together or not at all, so the manifest always describes the state its name holds. A restore reads both names in one `snapshot` and refuses a checkpoint whose versions or roots disagree (`inconsistent`).

**The manifest** (format version 2; every integer little-endian, every label one length byte and 1 to 32 printable ASCII bytes):

| Field | Meaning |
|---|---|
| `MIND-CKP`, format version (u16) | another format version is refused by its number (`version`), never read as this one; format 1 is not converted |
| contract (label), contract version (u16) | the contract the component follows; a restorer accepts only its own |
| schema (u16) | the state's schema version |
| epoch (u64) | the generation of the instance that saved it: the version of the checkpoint's names it wrote |
| sequence (u64) | the consistency point: how many requests the state includes |
| state (CID, 36 bytes) | the state object's root |
| counts (5 × u8) | resources (at most 8), authorities (at most 8), effects (at most 16), clients (at most 16), messages owed (at most 16) |
| inbox (u64) | how far an input queue was read: the number its source gave the last message taken |
| sent (u64) | the last sequence given to a message, so none is given twice |
| resources (labels) | logical names of what the state refers to: services, names. Never a handle or an address (MC-6.11) |
| authorities (labels) | the rights the instance used, as text. Never a capability |
| effects (id u64, outcome u8, label) | the effect journal: 0 pending, 1 done, 2 failed |
| clients (label, operation u64, result u64, at u64) | each client's last operation applied, the result it was answered, and the sequence it was applied at |
| outbox (sequence u64, destination label, what label) | the messages owed to others, in the order given; their bytes are in the state |

A manifest decodes only if it is exactly one encoding: no bytes after the last field, counts within bounds, valid labels and outcomes, each client once with an operation from 1 applied at a sequence not past the manifest's, messages in increasing order none past `sent`.

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
- **Who is active.** An instance that takes requests one after another saves once before its first (a claim). An instance restored before that save is fenced at its own first save, so it never answers a request.
- **Operations (format 2).** A request may carry its client's operation ID, increasing from 1.
  - An ID above the client's last is new: applied, and recorded with its result in the same save.
  - The ID equal to the last is a repeat: the client lost the answer. It is answered with the recorded result and not applied again.
  - A lower ID is stale and refused.
  - The manifest keeps the 16 clients applied most recently; another drops the one applied longest ago. A dropped client's next operation is taken as new, so repeats are told only for those 16.
- **The inbox and the outbox (format 2).**
  - A component that reads a numbered source keeps the number of the last message taken (`inbox`), in the save that applies it.
  - A message to another is owed in the save that decides it (`send`), and sent after that save. It stays owed until its receiver acknowledges it (`delivered`, saved), and after a restore it is sent again.
  - The receiver tells a repeat by the sender and the sequence, with its own clients table. So a message is applied once, and needs no reconciling, unlike an effect.
- **Rebinding (MC-6.11).** Restore returns the recorded authorities as text. A restored instance takes its rights from its current grants: what `init` and the shell give it now, never from the checkpoint. `rebind` lists the recorded authorities the instance does not hold now. Protected actions that need them are refused or degraded, and the checkpoint cannot return a revoked right.
- **Retention.** The checkpoint's names keep their last 4 versions, and those retain their manifests and states ([303](../../issues-done/303-retention-and-collection.done)). Recoverability is promised for the last 4 checkpoints, under the store's own durability (on the RAM disk, until the next reset). The code that reads them, `tally.elf`, is on the boot volume. Its schemas are versioned in the manifest.

## The contract of `tally`, version 2

`tally` keeps named counters (`tally add <key> [n] [op <client> <id>]`). Each run is a new instance: it restores, applies one request and saves.

| Item (MC-6.10) | `tally` 2 |
|---|---|
| Composition of state | Counters, at most 32 keys of at most 16 printable bytes. The object is one raw block of lines `key=count`, sorted by key (schema 1) |
| Consistency boundary | One request (`add`, the two saves of an `effect`, `reconcile`); `sequence` counts them |
| Dependencies | The block store. A resource is recorded as `blockstore` |
| Authorities used | `blockstore` (the shell's client) and `files` (for effects) |
| Permissible loss | A request whose save did not return `SAVED`. Nothing after `SAVED` (to the store's durability) |
| External effects | `effect <file>` writes `ram:<file>`. Its intent is saved first; `reconcile <id> done\|failed` settles a pending one, and no new effect begins while one is pending |
| Verification | Restore checks the manifest's format, contract, schema and epoch against the names, and every block against its CID. `tally show` prints the epoch, sequence, counters and pending effects |
| Fencing | A save from a stale epoch prints `FENCED` and writes nothing |
| Repeated requests | An addition with an operation ID is applied once: sent again it prints `REPEATED <client> <id>: ANSWERED <count>, NOT APPLIED AGAIN`, and an older ID is refused. One without an ID is applied each time |
| Format 1 | A checkpoint of format 1 is refused and named (`a checkpoint of format 1; this build reads format 2 and converts none`) |

## Evidence

- `tests/checkpoint_host.rs`, through the block store's own logic on a simulated medium:
  - the manifest's one encoding, and its refusals (every truncation, a trailing byte, another magic, format or outcome, counts past bounds);
  - save and restore after a remount;
  - a stale instance fenced;
  - another contract, contract version or schema, a state name moved alone, and a manifest that is not one, each refused;
  - effects coming back pending and reconciled, and a full journal keeping every pending effect;
  - authorities checked against what an instance holds;
  - format 2 (313-STO-0026): a client's new, repeated and stale operations and the oldest client dropped; the outbox bounded; a client twice, an operation 0, a client past the sequence, a message out of order or past `sent` each refused; format 1 refused by its number, also at a restore;
  - a model service stopped after each number of the store's calls in turn, its answer also lost after the commit: each of five operations applied once, every answer to it the count just after it, a stale one refused;
  - three owed messages delivered with a stop after each number of calls in turn, then again until none is owed: each received once, and the next sequence above them;
  - a save before the first request claims the component, and the instance restored before it is fenced.
- The QEMU `store` suite (x86 and aarch64):
  - `tally` counts across instances, and the checkpoint's two names move together;
  - an instance that restored epoch 3 and stalled is fenced when another saves epoch 4;
  - an effect cut short (its file written, its outcome not recorded) comes back pending, new effects are refused until it is reconciled, and the next effect completes;
  - an addition sent again with the same operation is answered with its first result and not applied, also after the store restarted, and an older operation is refused.

These are tests of one pilot, not a proof (MC-12.2). Not provided:
- a service with queues that uses format 2: the parts are in `mind::checkpoint` and checked on a model, and `tally` uses operation IDs alone. The first users are planned: the agent's sessions (260) and the messenger's client (950). A message's sending and acknowledgement are the model's, not the platform's;
- migration of an instance to another place;
- a test that a recorded authority the instance lacks is refused on the platform (the shell lends `tally` both rights; the refusal is host-tested).
