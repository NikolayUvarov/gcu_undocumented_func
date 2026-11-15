# 801-EFF-0036 — The last heartbeat within init's notice, however long a request hangs

**Type:** session · **Owner:** `EFF` · **Priority:** P1 · **Status:** in progress (2026-11-15) · **Blocked by:** — · **Main task:** [801](801-the-bench-stays-manageable.md) (S5) · **Constitution:** MC-6.5, MC-12.1

## Problem

KRN's finding of 2026-11-15, from round 140's gate (x86: network, TLS and Effector, on i9; `EFF/2026-11-15T1015Z-KRN-…` on `internal-dev-messages`):

- **What happened:**
  - the agent had ended itself during the suite's server outage, and `init` started it again as PID 205 at 349.9 s;
  - it connected at 351.05 s and logged nothing more;
  - `svc stop effector` came at 356.19 s, `init` killed it at 358.19 s (`KILLED AFTER THE NOTICE`), and no `shutting_down` heartbeat reached the server.
- **Why:**
  - the session reads the stop notice once per pass of its loop;
  - that pass waited on a request no answer came to, which the outage had left hanging, for up to its 10 s;
  - so the notice's 2 s passed first.
- **A real server or network can hold a request the same way.**
- **The suite missed it.** It required `[INIT] STOP NOTICE TO effector` and `ENDED WITHIN THE NOTICE` anywhere in `init`'s log, and an earlier stop's lines were there.

## Plan

1. **The notice worker marks the notice's reason on a page it shares with the session,** before it sends its message.
2. **Every wait of the session's connection reads that mark,** for a byte to send or to receive, every 2 ms. On it the wait ends at once, and the request fails.
3. **The last heartbeat goes at once,** before any further request and after a request cut short:
   - on a new connection when the old one was cut mid-way;
   - with no reconnect pause;
   - with 1.5 s for it all, connection included.

   The agent then ends, within the notice, whether the server answered or not.
4. **The `effector` suite reproduces the case:**
   - the fixture holds a poll 5 s;
   - `svc stop effector` comes while it waits;
   - the suite reads only `init`'s lines after `STOP NOTICE TO effector (PID <the agent's>)`, and fails on `KILLED AFTER THE NOTICE`.

## Acceptance criteria

- [x] The fixture counts a held request (`tests/effector_server_test.py`). LEG9, 2026-11-15: 15 tests pass.
- [x] The `effector` suite, x86 and aarch64: stopped while a poll hangs, the agent ends within the notice with its `shutting_down` heartbeat at the server. LEG9, 2026-11-15: x86's suite alone ended with status 0; aarch64's group passed, with `PASS: stopped by init while a poll hung past the notice, the agent heard the stop notice on its worker, cut the poll short, sent shutting_down and ended within the notice; started again, it is back`.
- [ ] Round 140's case does not come back in the gates that follow.

## Related

[750-EFF-0006](750-EFF-0006-session-service.md), [801-EFF-0028](801-EFF-0028-the-agent-comes-back-by-itself.md), 750-KRN-0194.
