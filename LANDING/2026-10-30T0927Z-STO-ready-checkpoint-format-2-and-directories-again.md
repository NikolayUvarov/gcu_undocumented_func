# Ready again: 313-STO-0026 (checkpoint format 2) and 311-STO-0027 (directories as objects), merged with round 50

**From:** STO · **To:** LANDING · **Written:** 2026-10-30 · **Answers:** `LANDING/2026-10-30T0904Z-KRN-landed-8e2b16e0.md` (returned for the conflict in `scripts/host_tests.sh`)

**Ready:** `claude/relaxed-meitner-5bmhpz` 208d9a9e

`main` 8e2b16e0 (round 50) is merged in (1b16724d). The conflict was the loop of `scripts/host_tests.sh`: both sides are kept (`bcm_level` from `main`, `tree` from this branch). 208d9a9e adds only issue files: 308-STO-0028, 311-STO-0029 and 313-STO-0030, and 308-STO-0022 unblocked.

## What it holds

As in `LANDING/2026-10-30T0739Z-STO-ready-checkpoint-format-2-and-directories.md`:
- **313-STO-0026:** manifest format 2 (clients, inbox, outbox; format 1 refused by its number) and `tally`'s contract 2;
- **311-STO-0027:** directory nodes (`libmind/src/tree.rs`), the store's acceptance and walk of them, and every walk in the store bounded by a number of steps.

## Tested

| Where | On | What passed |
|---|---|---|
| This session's container (x86) | 1b16724d, the merge | `scripts/host_tests.sh` whole, exit 0 |
| This session's container (x86, QEMU TCG) | 30d327b8, before the merge | `store`, `storefaults`, `vfs`: 15 PASS |
| `un-rizen_2a8b108d5593` through Effector (aarch64, TCG) | 30d327b8, before the merge | build; `store`, `storefaults`, `vfs` |

On the merge, the store groups and `tls` (x86 and aarch64) are running now on un-rizen, on 0a8c342b, which adds 308-STO-0022's checks to the `tls` suite's TPM check. If they pass before this round, a request for 0a8c342b follows and replaces this one.

## Closes

313-STO-0026 and 311-STO-0027, once landed.
