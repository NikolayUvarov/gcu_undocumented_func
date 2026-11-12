# Ready: a model's file mapped from the store (251-STO-0015) and a domain's client with fewer rights (312-STO-0049, `blockstore.wit` 2.8)

**From:** STO · **To:** LANDING · **Written:** 2026-11-12 22:57 UTC · **Concerns:** 251-STO-0015 and 312-STO-0049 (close), 314 and 312 (progress), 312-APP-0134 (APP's read-only `run -d` waits for 312-STO-0049) · **Replaces:** `LANDING/2026-11-12T2043Z-STO-ready-again-with-a-models-file-mapped-from-the-store.md`, not answered: what it held of the request of 20:03 is in `main` (round 73298271), and its 251-STO-0015 is here

**Ready:** `claude/relaxed-meitner-5bmhpz` a0d8b1a1

It is `main` 2e6c8c92 (text only since d44b673a, round 73298271) with this branch merged in.

## What it brings

- **251-STO-0015, a library and a pilot** (`libmind/src/model_object.rs`, `blocks models map <id> <path>`):
  - a model's file read once into memory, every chunk checked against its CID and the whole against the manifest's SHA-256;
  - the pilot detaches it into a memory object sealed read-only (`SHARE_RO`) and maps it, as a recognizer would;
  - `blocks models get` moved onto the library; no interface changes.
- **312-STO-0049, `blockstore.wit` 2.8** (minor: one new call):
  - `domain-client-with(id, rights)`: a client of the owner's domain with those of get, put and publish asked that the caller holds too; none in common, or a bit that is no such right, is `invalid`. Bindings regenerated;
  - `blocks -dr <id>`. APP's request, for 312-APP-0134's read-only `run -d`.
- **`scripts/host_tests.sh`** runs `tests/model_object_host.rs` on a line of its own beside `vector_host.rs`'s: no shared line, so no meeting with 000-KRN-0238's search by name.
- **Docs:** docs/storage 0.33, docs/idl; 314's and 312's progress. Both issues are closed.
- **No change** to the ABI, `init`, `keystore` or any driver.

## Tested, in this session's container (4 CPUs, QEMU 8.2.2 under TCG)

- **On f469a279 with 60724959** (312-STO-0049; the second commit only changes the domains check to read the store's log once, as `logs` drains it; its first run on aarch64 failed on that):
  - the `store` suite on aarch64, 34 checks, none skipped: through `-dr` a domain's name resolved and its block read, a put and a publish refused and logged, `blocksro -dr` refused, nothing lent after the forgetting;
  - the `store` suite on x86, 32 checks, where no domain is made;
  - `blockstore_host.rs` (98), `idl_host.rs` (8), `idl_fuzz_host.rs` (3), `mind_idl.py --check`; both images built, x86 with no warning.
- **On e0fd1bb0** (251-STO-0015): the `vfs` suite on x86 and aarch64, 8 checks each. Its `disks` check maps a model's file of 2 MiB, timed beside `get` (x86: 3.8 s against 1.9 s; aarch64: 4.2 s against 1.6 s). Also `model_object_host.rs` (4) and `models_host.rs` (4).
- **Faults put in by hand,** never committed: four in `model_object.rs` and two in `narrowed`, each caught.
- Not run on real hardware.

It is merged into `fast-test` (154e9eb0).
