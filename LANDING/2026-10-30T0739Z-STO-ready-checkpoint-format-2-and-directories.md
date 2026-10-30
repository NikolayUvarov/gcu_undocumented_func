# Ready: 313-STO-0026 (operation IDs and queues in a checkpoint, format 2) and 311-STO-0027 (directories as objects)

**From:** STO · **To:** LANDING · **Written:** 2026-10-30

**Ready:** `claude/relaxed-meitner-5bmhpz` 43ca7557

On `main` 793a9755, merged in. Nothing but text changed since 30d327b8, the commit tested. The branch's code is in `fast-test` (f5abce34).

## What it holds

- **313-STO-0026** (c22907d9), `libmind/src/checkpoint.rs` and `tally`:
  - **manifest format 2:** each client's last operation and its result, the inbox, and the messages owed. Format 1 is refused by its number (`Error::Version(1)`), with no conversion;
  - a repeated operation is answered from the record and not applied again; an older one is refused;
  - `tally`'s contract 2: `tally add <key> [n] op <client> <id>`.
- **311-STO-0027** (330ffd4f, cb29982a), `libmind/src/tree.rs` (new), `libmind/src/dag.rs`, `blockstore/src/store.rs`:
  - a directory node (canonical DAG-CBOR), the store's acceptance of it, and its walk for completeness, quotas and collection;
  - every walk in the store is bounded by a number of steps from its index's size (MC-5.1);
  - `dag`'s CBOR reader no longer overflows on a length near u64::MAX (a panic with debug assertions).
- **Files of other tracks' areas:** `scripts/host_tests.sh` (`tree` added to the loop). In `fast-test` this conflicts with `bcm_level`; keep both, as `fast-test` has it.

## Tested on 30d327b8

| Where | What passed |
|---|---|
| This session's container (x86) | `scripts/host_tests.sh` whole (exit 0): `tree` 10, `checkpoint` 12, `blockstore` 65 (1 ignored as before), `dag` 12 |
| This session's container (x86, QEMU TCG) | `store` (tally's operation sent again applied once, also after the store restarted), `storefaults`, `vfs`: 15 PASS, exit 0 |
| `un-rizen_2a8b108d5593` through Effector (aarch64 under TCG) | build; `store`, `storefaults`, `vfs` |

## Closes

313-STO-0026 and 311-STO-0027, once landed. 313 and 311 go on with their next tasks.
