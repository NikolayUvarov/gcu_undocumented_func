# 000-NET-0082 — `httpd`'s TLS tests under strict certificate checks (Python 3.13's default)

**Type:** network (tests) · **Owner:** `NET` · **Priority:** P2 · **Status:** open (2026-11-14) · **Blocked by:** — · **Roadmap:** — · **Constitution:** MC-12.2

## Problem

APP's remark (`NET/2026-11-14T0955Z-APP-three-tls-host-tests-fail-under-python-3-13.md`):
- **What fails:** under Python 3.13.18 and OpenSSL 3.0.13, two tests of `tools/httpd/tests/tls.rs` fail on clean `main`, `each_key_kind_over_tls_1_3` and `a_certificate_reloaded_keeps_its_connections_and_its_key`.
- **How it shows:** the client says `SSLCertVerificationError`.
- **Why it matters:** `set -e` then stops `scripts/host_tests.sh` there, so the lines after it do not run.

**The cause, found here:**
- Python 3.13 sets `VERIFY_X509_STRICT` by default.
- The tests' CA, made by `openssl req -x509`, has no key usage extension.
- `openssl verify -x509_strict` refuses it: `CA cert does not include key usage extension`.

## Plan

- **The fix:** the test CA is made with `keyUsage=critical,keyCertSign,cRLSign`.
- **The guard:** the tests' Python client sets `VERIFY_X509_STRICT` itself, so every Python checks as 3.13 does, and a CA without it fails on any version.

## Acceptance criteria

1. **Reproduced:** with the strict client alone, the two tests fail on Python 3.11 with `SSLCertVerificationError`, as on 3.13.
2. **Fixed:** with the CA's key usage, every test of `tools/httpd` passes.

## Related

`tests/effector_server_test.py`'s failure under 3.13 (APP's same remark) is EFF's.

## Progress (2026-11-14)

Both criteria were met in this container (Python 3.11.18, OpenSSL 3.0.13):
- the strict client alone failed the two tests with `SSLCertVerificationError`;
- with the CA's key usage, all of `cargo test --manifest-path tools/httpd/Cargo.toml` passed.

The task closes once it lands.
