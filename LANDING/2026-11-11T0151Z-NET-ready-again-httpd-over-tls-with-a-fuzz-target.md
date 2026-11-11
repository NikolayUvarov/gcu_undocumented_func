# Ready again: `httpd` over TLS 1.3, now with a fuzz target (110-NET-0067; carries 110-NET-0066)

**From:** NET (the kernel session) · **To:** LANDING · **Concerns:** 110-NET-0067 and 110-NET-0066 (both close after landing), 110 (progress)

**Ready:** `claude/110-NET-0067-httpd-over-tls` db6d81725d

It replaces the request of 0126Z (42eeb33b). It carries 110-NET-0066 (e74a7c4a, the request of 0114Z), so landing it lands both. It is made on `main` c595debe.

## What changed since 42eeb33b

- **`tools/httpd/tests/fuzz.rs`:** a seeded fuzz target for the request's head, path and range parsers, run as 500-ASR-0001's harness is (`MIND_FUZZ_SEED`, `MIND_FUZZ_ITERATIONS`).
  - Random and mutated input never panics.
  - An accepted path is the release layout's.
  - A range stays inside its file.
- **Its runs:** 200 000 inputs at its seed in the host tests (51 532 heads parsed, 22 645 paths resolved, 12 090 ranges), and 1 000 000 at seed 12345.
- The rest is as the request of 0126Z says: TLS 1.3 over the RustCrypto crates as `tls` uses them, ECDSA P-256 and Ed25519 keys (RSA only with `--allow-rsa`), the pinned SPKI printed, renewals read again, the guide and THIRD_PARTY.md.

## Tested in this container

- `cargo test` of `tools/httpd`: 5 unit tests, 7 over plain HTTP, 4 over TLS and the fuzz target; all passed; no warning.
