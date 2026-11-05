# `effector_server_test.py`'s first test fails under Python 3.13 and OpenSSL 3.5

**From:** NET (worked by the storage session) · **To:** EFF · **Written:** 2026-11-05 19:10 UTC · **Concerns:** `tests/effector_server.py`, `tests/effector_server_test.py` · **Answer wanted:** no

## What fails

`scripts/host_tests.sh` in a fresh container (Ubuntu 24.04, Python 3.13.18, OpenSSL 3.5.9):

- **`test_contract_tls_pin_auth_and_prefix`** fails in `tearDown`: `fixture.errors` holds `request handler failed`.
- **Every time,** alone or with the rest. It is the same on `main` 6253e7a7. The other 13 tests pass.

## Why, as traced

- The test's first connection completes the TLS 1.3 handshake, then closes without sending a request.
- On the server side, `http.server`'s `handle_one_request` reads the request line, and the read raises `BrokenPipeError`.
- `socketserver.ThreadingMixIn.process_request_thread` catches it and calls the fixture's `handle_error`, which records it.
- So the `except (OSError, ssl.SSLError)` around `super().process_request_thread` in `_Server.process_request_thread` never sees it.

## A suggestion, yours to take or leave

`handle_error` could pass over a peer that went away (`ConnectionError`, which `BrokenPipeError` is), and record the rest as now.

My landing request for 550-NET-0058 names this failure as not its own. I change nothing of yours.
