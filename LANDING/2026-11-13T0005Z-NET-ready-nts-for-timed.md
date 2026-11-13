# Ready: NTS for timed (111's stage 2): tls.wit 1.4, socket.wit 2.12, timed with NTS

**From:** NET (the kernel session) · **To:** LANDING · **Written:** 2026-11-13 00:05 UTC

**Ready:** `claude/111-NET-0073-tls-alpn-exporter` ece89c7d71

- **Advances:** [111-NET-0073](../../main/issues/111-NET-0073-tls-alpn-and-the-exporter.md) (`tls.wit` 1.4: `connect-alpn`, `export`), [111-NET-0074](../../main/issues/111-NET-0074-a-grants-host-names.md) (`socket.wit` 2.12: a grant's host names, and a name that waits for the network added once it resolves), [111-NET-0075](../../main/issues/111-NET-0075-timed-with-nts.md) (`timed` with NTS, RFC 8915; `init` lends it a TLS client without the device right and starts it right after `tls`, boot images and handoff unchanged). New third-party crates: RustCrypto's `aes-siv`, `cmac`, `dbl` (THIRD_PARTY.md).
- **Merged:** `main` 60b995bd, with no work of another track that has not landed. It merges with NET's sshd request (`claude/112-NET-0072-sshd`) without a conflict (checked with `git merge-tree`).
- **Tested:**
  - host: `tests/tls_exporter_host.rs` (OpenSSL's exporter vectors), `tests/nts_host.rs` (RFC 5297, NTS-KE, the fields, fuzzed), `sntp`, `services`, `boot_abi`; the IDL test;
  - QEMU in this container: `--suites nts` on x86 and aarch64; `timed` against two `chronyd` 4.5 NTS servers (a local script, not a gate test);
  - the maintainer's machines at 8869cfda: "x86: network, TLS and Effector" on x32-2 (1342 s) and "aarch64: network, TLS and Effector" on acc-2 (1153 s), both passed.
- **`scripts/host_tests.sh`:** two Cargo packages, `tests/nts/` and `tests/tls_exporter/`, on lines of their own, their names in `OWN_LINES`.
