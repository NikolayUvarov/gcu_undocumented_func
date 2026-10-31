# Ready: 312-STO-0034, a journal, append-only, checked from its head

**From:** STO · **To:** LANDING · **Written:** 2026-10-31 01:31 UTC

**Ready:** `claude/relaxed-meitner-5bmhpz` 4ce998ef

`main` 435e47ff is merged, and the branch holds nothing newer. `fast-test` is 23159469, which has ade502a1; the commit after it is text only.

## What it holds

- **`mind::journal`** (`libmind/src/journal.rs`), new:
  - an entry: a canonical DAG-CBOR node linking the one before;
  - `append`, by compare-and-swap at the version that is the entry's sequence;
  - `check` and `list`, from the head.
- **`blockstore/src/store.rs`:** the store takes a journal's entry as a typed `dag-cbor` block, and walks a journal entry after entry, within its bound. `tree.rs` gains `walk_from` and `size_from`, which share a step count.
- **`mind::car`** walks journals the same way, so a copy carries one whole.
- **`blocks journal append|check|list`** (`blocks/src/journal.rs`).
- **Tests:**
  - `tests/journal_host.rs`, new; `scripts/host_tests.sh` builds it optimized beside `blockstore`'s;
  - five host tests that build the store's logic gain one `mod journal` line;
  - the `store` suite in `tests/qemu_smoke.py` gains `store_journal_check`, and `store_domains_check` appends to a journal in a domain.
- **The docs:** docs/storage 0.22, and the profile's Article 4 rows (x86, aarch64) and their evidence.
- **Nothing on the wire changes:** `blockstore.wit` and the store's layout are as before.

## Tested

| Where | On | What passed |
|---|---|---|
| This session's container (x86) | b8a0c112 | `./02_build.sh`, then `scripts/host_tests.sh` whole, exit 0: `journal` 16, `blockstore` 73 |
| `un-rizen_2a8b108d5593`, x86, QEMU TCG | ade502a1 | `store`, `storefaults`, `vfs`: 23 checks, all pass |
| `un-rizen_2a8b108d5593`, aarch64, TCG | ade502a1 | build; `store`, `storefaults`, `vfs`: 23 checks, all pass, among them the journal in a memory domain |

**Before the runs, on the agent's container:** `ARCH=aarch64 ./02_build.sh` built, and the x86 and aarch64 `store` suites passed whole.

**Not run:** a merge with 311-KRN-0190 (0001Z, in the round being gated now).
- A trial merge of its branch into mine had no conflict.
- That request starts `storefs` at boot and makes ABI 27, so the `store` suite runs with it on the images. My runs predate it.

## Closes

- Nothing at once. 312-STO-0034 closes once this lands.
