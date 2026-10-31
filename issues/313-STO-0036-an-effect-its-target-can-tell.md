# 313-STO-0036 — An effect its target can tell: reconciled at restore by asking the target, never tried again blindly

**Type:** storage (`libmind`, `tally`) · **Owner:** `STO` track · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [313](313-persistent-actors.md) · **Constitution:** MC-6.6, MC-6.10, MC-6.12, MC-7.1, Appendix B.4

Split from [313](313-persistent-actors.md) (2026-10-31): "an effect journal (the intent before an effect, the result after); an indeterminate outcome reconciled, never blindly repeated", and its host criterion "a stop at every point between an effect and its record, each reconciled".

## Problem

`mind::checkpoint` keeps an effect journal (306): the intent is saved before an effect, its outcome after. An instance that ends between the two leaves the effect pending, and its outcome is unknown. Today only a person settles it: `tally reconcile <id> done|failed`. A component cannot settle one by itself, because nothing it recorded lets it ask the target whether the effect happened. Trying the effect again could do it twice (MC-6.6, B.4).

## Plan

- **An effect with a key** (manifest format 3): an effect's record may carry what its target tells it by, saved in the same save as the intent. The key is generic: a target's label and an operation the target applies once and tells afterwards. One target is built here: **a publication by compare-and-swap**, which is a name, the version it publishes from, and the root it publishes. A target that keeps operation IDs, as a component's clients table does (313-STO-0026), is the same shape, and later tasks may add one.
- **Asking the target** (`checkpoint::reconcile`, over a `Target` the caller gives). For a publication:
  - **Done:** the name is at `from + 1` with the key's root, or a later version and its history holds `from + 1` with that root.
  - **Not done:** the name is still at `from`. The effect may be done now by the same compare-and-swap. If another instance did it first, the swap is refused, and asking again says done. Whichever comes first, the name moves to that root at `from + 1` once.
  - **Failed:** `from + 1` holds another root, or the name was removed. Another writer changed it, so the effect is settled failed and never tried again.
  - **Unknown:** the name is past `from + 1` and its history no longer holds that version (the store keeps 4). The effect stays pending for a person, as one without a key does.
- **At restore,** a component reconciles its pending effects with keys before it applies a request, and saves what it learned. Fencing (MC-6.12) holds as before. An instance that recorded the intent and stalled is fenced at its next save. Its late swap from `from` is refused, because the name has moved, or it is the one swap that lands.
- **An effect without a key** stays as it is: pending until a person reconciles it.
- **Manifest format 3:** an effect's entry gains its key: a kind byte (0 none, 1 publication), then for a publication the name (1 to 64 bytes, the store's names), `from` (u64) and the root (a CID). Format 2 is refused by its number, and none is converted.
- **The pilot, `tally` 4:**
  - `tally publish <name> <text>` puts the text as a raw block and publishes it under `<name>` from the version it reads, as an effect with a key. It prints `EFFECT <id> DONE: <name> AT <v>`.
  - `crash-before` (after the intent's save, before the swap) and `crash-after` (after the swap, before the outcome's save) end the instance there.
  - Each run first reconciles what is pending. It prints `RECONCILED <id> DONE: <name> AT <v>`, `RECONCILED <id> NOT DONE: PUBLISHED NOW AT <v>`, `RECONCILED <id> FAILED: <name> CHANGED BY ANOTHER WRITER` or `RECONCILED <id> UNKNOWN: <name> PAST ITS HISTORY; RECONCILE IT BY HAND`.
  - Contract 3 is refused by its number. `effect <file>` and `reconcile <id> done|failed` stay for effects without a key.

## What it does not give, said in checkpoints.md

- **One target:** a name in a store. A publication in the component's own store could instead go into the checkpoint's own commit (304), and then nothing is left to reconcile. The keyed effect stands for a target outside that commit: here a name in another memory domain or the same store's, later another store or a service that keeps operation IDs.
- **No target that cannot tell:** a file written through `vfs` keeps no version, so `effect <file>` still needs a person.
- **History bounds the answer:** past 4 versions of the name, the outcome is unknown again.

## Acceptance criteria

- **Host** (`tests/checkpoint_host.rs`), through the block store's logic on a simulated medium:
  - a keyed effect stopped after each number of the store's calls in turn, from the intent's save to the outcome's. A new instance reconciles each stop. The name ends at `from + 1` with the root, once, never two versions, and the effect is settled done;
  - the same with the stopped instance resuming late at each point: its swap is refused or is the one that lands, and its save is fenced;
  - another writer at `from + 1` gives failed; a name past its history gives unknown, left pending;
  - format 3's one encoding, and its refusals (every truncation, a key of another kind, a name past 64 bytes); format 2 refused by its number, also at a restore.
- **QEMU `store` suite (x86, aarch64):**
  - `tally publish` with `crash-before`: the next run reconciles `NOT DONE`, publishes, and `blocks history` shows one version more;
  - with `crash-after`: the next run reconciles `DONE` and publishes nothing again;
  - a name published by `blocks publish` between the two: `FAILED`.
- **docs/storage/checkpoints.md** (version 5: format 3, `tally` 4, what it does not give) and the profile's Article 6 rows.

## Related

[313](313-persistent-actors.md), [306](../issues-done/306-checkpoints-and-rebinding.done), [313-STO-0026](../issues-done/313-STO-0026-operation-ids-and-queues-in-a-checkpoint.done), [304](../issues-done/304-several-names-at-once.done), [308-STO-0023](../issues-done/308-STO-0023-memory-domains.done).
