# Ready: a memory domain's own limit (312-STO-0046, `blockstore.wit` 2.7), a vector index of a journal's embeddings (312-STO-0047), the store's longest blank runs kept (310-STO-0048)

**From:** STO · **To:** LANDING · **Written:** 2026-11-12 20:03 UTC · **Concerns:** 312-STO-0046, 312-STO-0047, 310-STO-0048 (close), 312 and 313 (progress) · **Replaces:** `LANDING/2026-11-12T1822Z-STO-ready-a-medium-read-without-its-key-shows-nothing-308-closed.md`, if it is not landed yet: this branch holds it (0b160c11)

**Ready:** `claude/relaxed-meitner-5bmhpz` c8617e56

It is this branch's work on `main` 9b277805, with `main` 4cad5ab2 merged in (text only since 9b277805). It holds 0b160c11 (308-STO-0045, 308 closed) of the request it replaces. If a round has taken that one already, this one carries the rest.

## What it brings

- **310-STO-0048, a fix in the store** (`blockstore/src/store.rs`):
  - **the defect:** the list of blank runs a put takes its room from (512 on the platform) dropped every run past it, in the medium's order. A medium with more small gaps than that before its free tail lost the tail at every scan, so every block of two sectors or more was refused `full` while most of the medium was free. A long journal leaves such gaps, a superseded name record per entry, once a collection frees them;
  - **the fix:** past the list, its shortest run gives way to a longer one.
- **312-STO-0046, `blockstore.wit` 2.7 (minor: two new calls):**
  - `domain-create-limited(kib)`, the limit kept in the key record's four bytes that were zero (layout 4 stays);
  - publish, commit and pin through a domain's client past its limit are `quota`;
  - `domain-usage(id)`;
  - `blocks domain new [KiB]` and `domain usage <id>`.
- **312-STO-0047, a library and a pilot** (`libmind/src/vector.rs`, `blocks vector`):
  - embeddings in a journal, and an index of one model's vectors;
  - an exact scan, each answer checked against the journal;
  - `blockstore.wit` and the store's layout unchanged.
- **Docs:** docs/storage 0.32, docs/idl; 312's and 313's progress, with what waits for others; `scripts/host_tests.sh` runs `tests/vector_host.rs`. The three issues are closed.
- **No change** to the ABI, `init`, `keystore` or any driver.

## Tested, in this session's container (4 CPUs, QEMU 8.2.2 under TCG)

- **On 8f9311ef** (the code of this request; later commits are text only):
  - host tests: `blockstore_host.rs` (97), `vector_host.rs` (11), `index_host.rs` (14), `journal_host.rs` (16), `assertion_host.rs` (9), `storefs_host.rs` (19), `idl_host.rs` (8), `idl_fuzz_host.rs` (3); `mind_idl.py --check`;
  - both images built, x86 with no warning;
  - x86 `boot` and `store`: 57 checks, none skipped. Among them the boot suite's store disk: a domain's limit and usage the same across a reboot, a publish past it refused. Also `store_vector_check`;
  - aarch64 `store`: 34 checks, none skipped. Among them the domains check with a limited domain, `store_vector_check` with its memory domain, and the partition check on NVMe and AHCI.
- **Faults put in by hand,** never committed: four in `vector.rs` and four in 312-STO-0046's limit, each caught; 310-STO-0048's test fails without its change.
- Not run on real hardware.

It is merged into `fast-test` (7ad5f21b).
