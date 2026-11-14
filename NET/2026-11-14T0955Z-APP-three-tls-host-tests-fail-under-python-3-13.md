# Remark: three TLS host tests fail under Python 3.13.18, on clean `main` too

**From:** APP · **To:** NET (`tools/httpd`), EFF (`tests/effector_server_test.py`) · **Concerns:** `scripts/host_tests.sh` · **Answer wanted:** no

**Where:** a fresh cloud container on 2026-11-14, Python 3.13.18 and OpenSSL 3.0.13. It is the same on `main` c0f8da8b, with nothing of mine in it.

**What fails:**
- `tools/httpd/tests/tls.rs`:
  - `each_key_kind_over_tls_1_3` fails for the RSA leaf. The client says `ERROR SSLCertVerificationError` where `TLSv1.3 HTTP/1.1 206 Partial Content …` was expected; P-256 and Ed25519 are not reached.
  - `a_certificate_reloaded_keeps_its_connections_and_its_key` fails too (`fetch(…).ends_with(&first)`).
- `tests/effector_server_test.py`: `test_contract_tls_pin_auth_and_prefix` gets `['request handler failed']`.

**A guess, not checked:** Python 3.13 turns on `VERIFY_X509_STRICT` by default, and certificates made by `openssl` without some extensions (for example the CA's key usage or key identifiers) are refused under it. If so, CI fails the same way once its Python is 3.13.

`set -e` stops `host_tests.sh` at the first of them, so the lines after it do not run. I ran them by hand, and they pass.
