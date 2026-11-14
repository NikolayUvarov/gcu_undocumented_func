# 801-EFF-0028 — The agent comes back by itself: no answer for a while restarts it, and a failing start is never quarantined

**Type:** session · **Owner:** `EFF` · **Priority:** P1 · **Status:** in progress (2026-11-14) · **Blocked by:** — · **Main task:** [801](801-the-bench-stays-manageable.md) (S5: "only a deliberate switch-off stops the agent"; "an agent that cannot reach its server says so on the screen and in the log, and keeps trying") · **Constitution:** MC-6.5, MC-12.1

## Problem

Three ways the agent lost its server for good, with the network working:

- **A spent flow grant.** `init` takes the agent's grant from `netpolicy` once, at its start. The grant has the owner's term and volume. The setups written so far said 86400 s and 1 GB, so after one day of uptime every TLS connection is `denied`, and the agent tried again every 30 s for ever. Only a restart would give it a new grant.
- **A restarted service.** When `tls` or `parse` restarts, the agent's clients of them are dead, and only a restart of the agent gets new ones.
- **A failing start.** `fail()` ended the process at once, for example with no grant because `netpolicy` was late, or a disk that did not answer. Three such ends in 60 s and `init` quarantines the agent (MC-6.5): it is then gone until a person starts it.

## Plan

1. **No answer for `restart-after`:**
   - `restart-after` is a new `effector.conf` line, 30–3600 s, 300 s by default.
   - Whatever the status, if no answer has come from the server for that long, the agent logs why and ends itself. `init` restarts it with a new flow grant and new clients. The journal keeps every answer across that.
   - It does not end while a measurement runs or while the machine restarts on purpose (750-EFF-0006's quiet time).
2. **A failing start waits 31 s before it ends,** so `init`'s budget of 3 restarts in 60 s is never reached and the agent is restarted for ever.
3. **On the screen and in the log:**
   - the gateway's mark stays grey while the agent tries (750-EFF-0015);
   - each try's failure is logged as before;
   - `THE SERVER ANSWERS AGAIN, AFTER N S (why it did not)` once it does, after 60 s or more without an answer.
4. **The owner's line.** The setup notes give `netpolicy` lines with the longest term and volume (`4294967295 18446744073709551615`). The agent no longer depends on it.

## Acceptance criteria

- [x] Host tests: `restart-after`'s bounds, default and duplicate (`tests/effector_config_host.rs`).
- [x] The fixture can stand for a server out of reach (`outage`), tested in `tests/effector_server_test.py`.
- [ ] The `effector` suite, x86 and aarch64. The server is out of reach for 75 s with `restart-after 40`, and:
  - the agent ends itself;
  - `init` restarts it, without quarantine;
  - its heartbeats come back once the server does.
- [ ] On a bench machine: an agent that met a spent grant or a restarted `tls` is back through hesere with no hand at the machine (MC-12.1).

## Related

[801](801-the-bench-stays-manageable.md), [801-EFF-0027](801-EFF-0027-commands-judged-by-the-servers-time.md), [750-EFF-0006](750-EFF-0006-session-service.md), [750-EFF-0015](750-EFF-0015-the-agents-mark-on-the-screen.md).
