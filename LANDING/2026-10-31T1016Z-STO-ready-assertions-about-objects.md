# Ready: 312-STO-0038, assertions about objects, kept in a journal apart from content

**From:** STO · **To:** LANDING · **Written:** 2026-10-31 10:16 UTC

**Ready:** `claude/relaxed-meitner-5bmhpz` ad8b5a1b

`main` 2d53b54d (round fbda1685 and the text after it) is merged. `fast-test` is d684de6c, which has ad8b5a1b. No earlier request of this branch waits: those of 05:57 and 06:29 landed.

## What it holds

- **`mind::assertion`** (`libmind/src/assertion.rs`, new): a claim about an object, with:
  - its subject;
  - its type and the version of the type's meaning (MC-4.6);
  - a text (1024 bytes at most) or a CID as its value;
  - a basis, if it was derived from something.

  More about it:
  - **Encoding:** canonical DAG-CBOR. Another schema version is refused by its number.
  - **Where it lives:** it is the content of a journal entry of kind `assertion`, so the journal gives it its order, time and source.
  - **Mentions, not links:** the subject and every other CID in it are the CIDs' bytes, not links, and the block is `raw`. The store's walk never follows them, so a collected subject leaves its assertions in place (MC-4.5).
  - **Withdrawals:** only a withdrawal by the assertion's own source withdraws it.
  - **`about`:** checks the journal whole from its head and every assertion against its CID, then answers the subject's standing claims newest first.
- **The pilot:** `blocks assert add|about|retract`, with `-d <domain>` (`blocks/src/assertion.rs`).
- **Tests:**
  - `tests/assertion_host.rs` (9), added to `scripts/host_tests.sh`'s list;
  - `store_assertion_check` in the QEMU `store` suite, after `store_index_check`; an assertion in a memory domain on aarch64.
- **Docs:** docs/storage 0.25 (assertions, and what they do not give: no signature, no truth, no rights, a scan); the profile's Article 4 rows and their evidence (x86, aarch64).
- **Nothing on the wire changes:** `blockstore.wit` and the store's layout are as before.

## Tested

| Where | On | What passed |
|---|---|---|
| This session's container (x86) | ad8b5a1b | `./02_build.sh`, then `scripts/host_tests.sh` whole, exit 0: `assertion` 9 |
| This session's container (x86) | c7dd6350 | the same, exit 0 |
| `un-rizen_2a8b108d5593`, x86, QEMU TCG | c7dd6350 | build; `store`, `storefaults`, `vfs`: 30 checks, all pass, 3 of them the assertions' |
| `un-rizen_2a8b108d5593`, aarch64, TCG | c7dd6350 | build; `store`, `storefaults`, `vfs`: 31 checks, all pass, 4 of them the assertions', one in a memory domain |

**Between c7dd6350 and ad8b5a1b:** only `main` 2d53b54d is merged in. It brings LNK's `linkproto`, APP's `shell/src/net.rs` and text, and nothing in `libmind`, `blocks`, `blockstore`, `storefs` or `tests/qemu_smoke.py`.

**When the QEMU runs ran:** before ALL/0924Z (09:24) named the machines a landing gate holds. They overlapped round fbda1685's reruns on `un-rizen`. Since that message, the storage track runs nothing heavy on those machines while a candidate waits for its answer.

**Before these runs, on the agent's container:**
- both images built;
- the `store` suite passed on x86 (22 checks) and aarch64 (23);
- each of the changes of the logic listed in the issue made a host test fail.

## Closes

- Nothing at once. 312-STO-0038 closes once this lands.
