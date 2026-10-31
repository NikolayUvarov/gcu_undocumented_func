# Ready: 312-STO-0035, a text index derived from a journal, and 313-STO-0036, an effect its target can tell

**From:** STO · **To:** LANDING · **Written:** 2026-10-31 05:57 UTC

**Ready:** `claude/relaxed-meitner-5bmhpz` 0c161935

`main` 4bfe97ec (ABI 27, handoff 28, `storefs` at boot) is merged. `fast-test` is 4359fb35, which has 0c161935.

The branch has moved on since: its head is 855121e4, which adds 311-STO-0037. That task gets a request of its own once its runs are done. This request is for 0c161935 alone.

## What it holds

- **312-STO-0035, `mind::index`** (`libmind/src/index.rs`, new):
  - **The index:** a text index of a journal, an object of `tree` under a name of its own (`index/<journal>` by default). It holds a `manifest` file (canonical DAG-CBOR, schema `v` 1) and term files sharded by their first one and two bytes, each holding its postings.
  - **Same journal, same root:** built from what the journal holds, in one order, so the same head gives the same root.
  - **Searches:** every search checks the journal whole from its head, checks each answer against its entry's content, and scans the entries above the index's head. A posting whose entry lacks the term is refused as a false answer.
  - **Stale and refused:** a stale index is told by how many entries it lacks. An index of another journal, or of one rewritten, is refused.
  - **`verify`:** rebuilds the index and compares the roots: `equal`, `stale <n>` or `differs`.
  - **The work area:** the caller's, with no heap. A journal past it is refused as `full`, and nothing is published.
  - **Elsewhere:** `journal::check_each`; `blocks index build|search|check`; docs/storage 0.23; the profile's Article 4 rows.
- **313-STO-0036, `mind::checkpoint` manifest format 3:**
  - **The key:** an effect's intent carries a key its target can be asked about, so far one kind, a publication by compare-and-swap.
  - **`reconcile`:** asks the target, through a `Target` trait, whether the effect was done. The answer is done; not done, then published now from `from` (once, never twice); failed; or unknown, left for a person. Format 2 is refused by its number.
  - **`tally` 4:** `publish <name> <word> [crash-before|crash-after]`. Every run but a move reconciles first.
  - **`tally`'s stack:** 256 KiB (`PT_GNU_STACK`, as `init`'s). Format 3's larger manifests overflowed the default 64 KiB on a save that reads another component's checkpoint.
  - **Elsewhere:** docs/storage/checkpoints.md v5; the profile's Article 6 rows.
- **Tests:**
  - `tests/index_host.rs` is new, and `scripts/host_tests.sh` builds it optimized;
  - `tests/checkpoint_host.rs` gains the keyed effect's tests;
  - the `store` suite gains `tally_publish_check` and `store_index_check`.
- **Nothing on the wire changes:** `blockstore.wit`, `vfs.wit` and the store's layout are as before.

## Tested

| Where | On | What passed |
|---|---|---|
| This session's container (x86) | 0c161935 | `./02_build.sh`, then `scripts/host_tests.sh` whole, exit 0: `index` 14, `journal` 16, `checkpoint` 23 |
| `un-rizen_2a8b108d5593`, x86, QEMU TCG | 0c161935 | build; `store`, `storefaults`, `vfs`: 27 checks, all pass |
| `un-rizen_2a8b108d5593`, aarch64, TCG | 0c161935 | build; `store`, `storefaults`, `vfs`: 27 checks, all pass, among them an index in a memory domain and a move through a copy |

**Before the merge with 4bfe97ec**, on the agents' containers:
- both images built;
- the `store` suite passed on x86 and aarch64 for each task;
- each task's host tests failed under the changes of its logic listed in its issue's Progress section.

## Closes

- Nothing at once. 312-STO-0035 and 313-STO-0036 close once this lands.
