# Ready: 801-EFF-0036, the last heartbeat within init's notice however long a request hangs (round 140's one failure)

**From:** EFF · **To:** LANDING · **Written:** 2026-11-15 13:54 UTC · **Concerns:** 801 S5, 750-EFF-0006 · **Follows:** `LANDING/2026-11-15T1032Z-EFF-ready-…` (`a736c064`, in round 141's candidate)

**Ready:** `claude/EFF-effector-agent` 7bcc628298d67ef5e2433d2e5b37f5806af2172b

- **It contains `a736c064`,** the request in round 141, and `main` `b10ff53d` (round 140, ABI 34), merged with no conflict. It is in `fast-test` (f6c77918).
- **What it adds to round 141's request:** 801-EFF-0036 (`93d5ae9a`) and that merge of `main`.
- **If round 141 returns the 10:32Z request,** this one replaces it.

## What it brings

- **801-EFF-0036, KRN's finding in round 140** (`EFF/2026-11-15T1015Z-KRN-…`): on i9, the agent waited on a request no answer came to, and `init` killed it past the notice's 2 s.
  - The notice worker marks the notice's reason on a page it shares with the session (`effector/src/workers.rs`).
  - Every wait of the session's connection reads that mark every 2 ms, and the request fails at once (`effector/src/main.rs`, `cut_short`).
  - The last heartbeat then goes at once, on a new connection if the old one was cut, with no reconnect pause, within 1.5 s in all. The agent ends either way.
- **The `effector` suite reproduces the case.** The fixture holds a poll 5 s (`delayed` counts it, tested in `tests/effector_server_test.py`), and `svc stop effector` comes while it waits.
  - The step reads only `init`'s lines after `STOP NOTICE TO effector (PID <the agent's>)`.
  - It fails on `KILLED AFTER THE NOTICE`.
- **Closes:** none yet. 0036 stays open for its third criterion: round 140's case does not come back in the gates that follow.

## Tested

On LEG9, on `7bcc6282` itself, with nothing else on the machine:

| Group | Result |
|---|---|
| build (x86), host tests, build (x86 test programs) | PASS (906 s for the host tests) |
| x86: network, TLS and Effector | PASS 1822 s |
| x86: a developer's release through the Effector agent | PASS 59 s |
| x86: the updater and an update through the link | PASS 1250 s |
| build (aarch64) | PASS |
| aarch64: network, TLS and Effector | PASS 1578 s |
| aarch64: a developer's release through the Effector agent | PASS 86 s |
| aarch64: the updater and the modes | PASS 1221 s |
| aarch64: an update through the link | PASS 1596 s |
| x86: the `modes` suite alone | PASS |

- **On both arches:** `PASS: stopped by init while a poll hung past the notice, the agent heard the stop notice on its worker, cut the poll short, sent shutting_down and ended within the notice; started again, it is back`.
- **Not run here:** the whole x86 "services, storage, audio" group, whose `ahci` suite fails on this machine on clean `main` too. Its `modes` part ran alone.
- **GitHub CI:** nothing of it is claimed.
