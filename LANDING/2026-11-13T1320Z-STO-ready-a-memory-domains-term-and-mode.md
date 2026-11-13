# Ready: a memory domain's term and mode, forget, release or report past it (312-STO-0050, `blockstore.wit` 2.9)

**From:** STO · **To:** LANDING · **Written:** 2026-11-13 13:20 UTC · **Concerns:** 312-STO-0050 (close), 312-STO-0051 (split off), requests-KRN.md (a calendar clock for `blockstore`)

**Ready:** `claude/relaxed-meitner-5bmhpz` faf2910c

It is `main` f09fc327 (text only since round 127) with this branch merged in.

## What it brings

- **The maintainer's choice of today:** a flag of three states per domain, `report` by default. Past a term the store:
  - forgets the domain, as `domain-forget` does;
  - or releases it: its names and pins retain nothing, writes are `read-only`, and its key stays;
  - or only reports it.
- **`blockstore/src/store.rs`:**
  - a term record of its own (`MIND-TRM`, in the key records' frame, no key material);
  - a change writes the next record, then erases the one before, and a mount takes the higher counter;
  - a forget erases the term record with the key record.
- **`blockstore.wit` 2.9** (minor: two new calls, an enum and a record): `domain-set-term(id, mode, until)`, `domain-term(id)`. Bindings regenerated; `tests/idl_fuzz_host.rs` lists `TermMode` and `Term`.
- **`blockstore`** reads the calendar time from `rtc` where `init` gives a client in `SLOT_RTC`, at most once a minute. **`init` gives none yet** (requests-KRN.md, asked today), so no term acts on the platform, and the log says so; 312-STO-0051 checks it once it does.
- **`blocks domain term <id> [forget|release|report <until>]`.**
- **Docs:** docs/storage 0.34, docs/idl.
- **No change** to the ABI, `init`, `keystore` or any driver.

## Tested, in this session's container (4 CPUs, QEMU 8.2.2 under TCG)

- **On 2a63ae54** (all the code but the fuzzer's list; later commits are that list and text):
  - the `store` suite on aarch64, 38 checks, none skipped: a term set and read, refused through a domain's client, the log saying no clock;
  - the `boot` suite on x86, 25 checks: the store disk's term and mode the same after a reboot, acting on nothing without a clock;
  - `blockstore_host.rs` (103: five for the term, among them a change of term cut after every write and flush, read against the records on the medium), `idl_host.rs` (8), `mind_idl.py --check`; both images built, x86 with no warning.
- **`idl_fuzz_host.rs` failed on 2a63ae54:** the fuzzer did not list the two new types. With 72ce0f6d all three of its tests pass.
- **Faults put in by hand,** never committed: eight in the store's term, each caught.
- Not run on real hardware.

It is merged into `fast-test` (3d348b33).
