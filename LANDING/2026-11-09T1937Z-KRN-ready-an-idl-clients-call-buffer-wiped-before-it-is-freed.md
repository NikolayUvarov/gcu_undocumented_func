# KRN ready: an IDL client's call buffer is wiped before it is freed (000-KRN-0226)

**Ready:** `claude/youthful-mendel-mf1soy` 004b57030fa5

**From:** the kernel session (KRN, PRT, NET)

**What it lands** (on `main` 9cd836e3):
- **a4e5f772, 000-KRN-0226 (new, APP's request of 2026-11-09):** `mind::mem::Wiped` (`libmind/src/wiped.rs`).
  - It wraps a call's `Pages` and overwrites them with zeros, by volatile word stores, when it is dropped, before the pages are freed.
  - `scripts/mind_idl.py` makes every buffered client's buffer a `Wiped`.
  - The 33 generated interfaces and `tests/idl/sample.rs` are regenerated.
  - The wire and the WIT do not change. Every program that uses a generated client is rebuilt with it.
- `tests/idl_host.rs` and `tests/idl_fuzz_host.rs` take `Wiped` from libmind's file. The profile's MC-1.8 line and `docs/idl/README.md` say so. APP's request is taken from `requests-KRN.md`.

**Tested** in the kernel session's container (QEMU 8.2.2, TCG, 4 CPUs), after fresh builds of the tree:
- **The host:**
  - `idl_host`, 7 tests, the new `a_clients_buffer_is_wiped_before_it_is_freed` among them; it failed with the sample regenerated without `Wiped`;
  - `idl_fuzz_host`, 3 tests;
  - `mind_idl.py --check`: every file up to date.
- **x86:** the `shell` and `net` suites, 54 checks, none skipped. The secret line's check and `get -u` are among them.
- **aarch64 `virt`:** the same suites, 51 checks; 3 skipped for x86's test programs, as always there.
- **Not run:** the other groups. Every program changes, so the gate's full run is the test of the rest.
