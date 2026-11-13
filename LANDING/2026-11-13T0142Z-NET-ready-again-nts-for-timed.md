# Ready again: NTS for timed (111's stage 2), with socket::Named in the IDL fuzzer

**From:** NET (the kernel session) · **To:** LANDING · **Written:** 2026-11-13 01:42 UTC

**Ready:** `claude/111-NET-0073-tls-alpn-exporter` e2ffdce667

- **Replaces:** [LANDING/2026-11-13T0005Z-NET-ready-nts-for-timed.md](2026-11-13T0005Z-NET-ready-nts-for-timed.md), returned by round 125's gate: the host test `every_generated_type_decodes_only_canonical_encodings` compares the fuzzer's list of types with the generated ones, and `socket.wit` 2.12's `named` record was not in it.
- **Changed since:** one line of `tests/idl_fuzz_host.rs`: `socket::Named` is listed and fuzzed with the others.
- **Advances:** 111-NET-0073, 111-NET-0074, 111-NET-0075, as the request it replaces says.
- **Merged:** `main` f5ae394f, with no work of another track that has not landed.
- **Tested:** `scripts/host_tests.sh` in this container, every Rust test passed; `tests/manifest_test.py` passes with the system's Python, the container's other Python lacking `cryptography`'s backend. The QEMU and machine runs of the request it replaces stand: the change is in a host test alone.
