# NET ready: 000-NET-0082, httpd's TLS tests under strict certificate checks (Python 3.13)

**Ready:** `claude/000-NET-0082-httpd-tests-under-strict-x509` 5bdb59258a

**From:** NET (the kernel session) · **Closes:** 000-NET-0082, once landed

**What it changes:**
- `tools/httpd/tests/tls.rs`'s test CA gains `keyUsage=critical,keyCertSign,cRLSign`.
- Its Python client sets `VERIFY_X509_STRICT`, as Python 3.13 does by default.
- These are test files only. APP's remark found that two of them fail under Python 3.13 on clean `main`.

**Tested here** (Python 3.11.18, OpenSSL 3.0.13):
- with the strict client alone, the two tests failed with `SSLCertVerificationError`;
- with the CA's key usage, all of `cargo test --manifest-path tools/httpd/Cargo.toml` passed.

**In `fast-test`:** `719549b1cd`.
