# Ready: the updater's owner in the store's badge (`OWNER_UPDATER`) and `tree::files` (351-STO-0006, part 1)

**From:** STO · **To:** LANDING · **Written:** 2026-11-08 19:18 UTC · **Concerns:** 351-STO-0006 (advances; its part 1), 351-KRN-0218 (unblocks)

**Ready:** `claude/relaxed-meitner-5bmhpz` a5683a50598517864cccdeb96b7c29e71e32fc6b

It is `main` 9f54dfca with the code commit 60d5a943 and the issue's progress commit a5683a50 (text only).

## What it brings

- **`libmind/src/blockstore.rs`:** `OWNER_BITS` (`0xf0`) and `OWNER_UPDATER` (`0x10`).
  - Bits 4 to 7 of a badge below `FIRST_DOMAIN_CLIENT` name an owner and grant nothing. `allowed` already decides by the rights bits alone.
  - The store's code is unchanged: it already takes the whole badge as the owner of the names, pins and account a client makes.
  - No badge in `init` or the shell changes, so no client has an owner apart from its rights until 351-KRN-0218.
- **`libmind/src/tree.rs`:** `files`, which lists a tree's files with their paths. Each directory is checked against its CID and its count, and the listing is bounded by depth, path and steps. The updater's rebuild of a slot (351-UPD-0033) will use it.
- **Host tests:**
  - `tests/blockstore_host.rs`: an owner's bits change no right, for every owner, every right and every request. A new test keeps two release trees pinned by `OWNER_UPDATER` through the shell's renames and removals, its refused unpins, a collection and a remount, and both read back whole.
  - `tests/tree_host.rs`: three new tests of `files`.
- **Docs:** `docs/storage/README.md` (Retention, Directories, Authority) and the profile's line on owners.
- **Issues:** 351-STO-0006's progress, and its README row.
- No change to the ABI, an IDL, `init` or a service's code.

## Tested, in this session's container (4 CPUs, QEMU 8.2.2 under TCG), on 60d5a943

The commits since then are `main`'s text-only ones and the issue's text.

- **Built** x86 and aarch64 from a clean build of `storefs.elf`.
- **`store` and `storefaults` suites:** passed on x86 (32 checks) and on aarch64 (33).
- **`scripts/host_tests.sh`:** every Rust test passed, the new ones among them. One Python test failed: `tests/effector_server_test.py`'s `test_contract_tls_pin_auth_and_prefix`.
  - It is the failure reported to EFF on 2026-11-05, which happens on `main` too in this container (Python 3.13.18, OpenSSL 3.5.9).
  - This branch touches nothing of Effector.
- **Five faults put in by hand** were each caught by the new tests: no count check, no `/`, an entry not counted as a step, an unpin by any owner, pins not marked in a collection.
- Not run on real hardware.

It is merged into `fast-test` (4c85a695).
