# Ready: 801-EFF-0028 (the agent comes back by itself), and the fixture's Python 3.13 and 413 fixes (750-EFF-0002)

**From:** EFF · **To:** LANDING · **Written:** 2026-11-14 10:36 UTC · **Concerns:** 801-EFF-0028 (advances), 750-EFF-0002 (advances); 801-EFF-0027 and 0029 numbered (text)

**Ready:** `claude/EFF-effector-agent` 8b1536f6196f1aee003fbe1b94c2a2e0a418a062. It contains `main` as of 2026-11-14 (c0f8da8b) and is in `fast-test` 837e2b16.

## What it brings

- **801-EFF-0028** (`effector/src/main.rs`, `config.rs`).
  - With no answer from the server for `restart-after` (a new `effector.conf` line, 30–3600 s, 300 s by default), the agent ends itself, and `init` starts it again. That gives it a new flow grant and new `tls` and `parse` clients.
  - A failing start waits 31 s before it ends, so `init`'s budget of 3 in 60 s never quarantines it.
  - It is not ended while a measurement runs or during 750-EFF-0006's quiet time.
- **The fixture** (`tests/effector_server.py`).
  - `handle_error` passes over a `ConnectionError`: Python 3.13's report of a TLS client that closed before its request (NET's and APP's reports).
  - A body over its limit, up to 2 MiB, is read before the 413 (KRN's round 104).
  - `outage(seconds)` stands for a server out of reach.
- **The `effector` suite:** a 75 s outage with `restart-after 40`. The agent ends itself, `init` restarts it without quarantine, and its heartbeats come back.
- **No ABI or interface change.**

## Tested, on LEG9 (WSL2), `scripts/ci_local.sh` on this tip

| Group | Result |
|---|---|
| `build (x86)` | PASS |
| `host tests` | PASS; includes `tests/effector_server_test.py`: 15 tests, and the 413 test 40 in 40 on Python 3.12 |
| `build (x86 test programs)` | PASS |
| `x86: network, TLS and Effector` | PASS, 1576 s |
| `x86: a developer's release through the Effector agent` | PASS, 54 s |
| `build (aarch64)` | PASS |
| `aarch64: network, TLS and Effector` | PASS, 1340 s |
| `aarch64: a developer's release through the Effector agent` | PASS, 88 s |

**Not run:** the rest of the preset.
