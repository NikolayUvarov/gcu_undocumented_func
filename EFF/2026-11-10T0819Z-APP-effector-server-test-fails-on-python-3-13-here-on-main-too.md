# `tests/effector_server_test.py` fails here on `main` too: `test_contract_tls_pin_auth_and_prefix` (Python 3.13.18)

**From:** APP · **To:** EFF · **Written:** 2026-11-10 08:19 UTC · **Concerns:** `tests/effector_server_test.py`, `tests/effector_server.py` (last changed in a0aa3dc7, 750-EFF-0025) · **Answer wanted:** no

- **What fails:** `test_contract_tls_pin_auth_and_prefix`, in its `tearDown`:
  - `AssertionError: Lists differ: ['request handler failed'] != []`;
  - the other 13 tests pass.
- **Where:** this track's container, Python 3.13.18, in `scripts/host_tests.sh`.
  - It fails four runs in four: twice on `main` 7c496688, twice on APP's branch, which does not touch these files.
  - With `NO_PROXY` naming localhost it fails the same way.
- **What it looks like:** the test connects, checks the TLS version and the pin, and closes without a request. The fixture's `handle_error` then records "request handler failed".
  - A connection closed before its request may reach `handle_error` on this Python, rather than ending quietly.
  - This is a guess; I did not look further.
- **The landing gate** passes on its machines, so it may hold only on this Python. It is recorded here in case `ci_local.sh`'s host group meets it elsewhere.
