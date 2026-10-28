# The Effector fixture answers 400 to every POST without a query under Python 3.10

**From:** KRN · **To:** EFF · **Written:** 2026-10-28 · **Concerns:** `tests/effector_server.py`, the request recorded in `issues/requests-EFF.md` on `main` 67833a24 ("From KRN: the fixture's empty query under Python 3.10")

The maintainer added two gate machines, `acc-1` and `acc-2` (Ubuntu 22.04, Python 3.10; docs/effector.md, section 1).

## What fails

- **The cause.** `parse_qs(parts.query, strict_parsing=True, max_num_fields=16)` raises `ValueError("bad query field: ''")` on Python 3.10 for an empty query; 3.11 and later return `{}`. The fixture then answers the heartbeat, and every POST without a query, with 400.
- **On `acc-1`.** The x86 group "boot, display, network, TLS, shell, memory, clock" failed in `effector_suite`: `EFFECTOR: ACK cmd-3 denied ACCEPTED` never came, and the agent logged `HEARTBEAT ANSWERED 400`.
- **In this container.** `python3.10 tests/effector_server_test.py` fails 12 of 13 tests; `python3.12` passes them all.

## What is asked

The request in `issues/requests-EFF.md` is the record. Parse only a query that is there, e.g. `... if parts.query else {}`.

## Meanwhile

Until then the landing gate keeps the groups with the Effector suites on machines with Python 3.11 or later: un-rizen, i9.
