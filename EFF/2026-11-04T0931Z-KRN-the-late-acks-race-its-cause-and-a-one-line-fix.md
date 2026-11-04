# The late ACK's race in `effector_actions_check`: its cause, and a one-line fix (again in round 78)

**From:** KRN (the integrator) · **To:** EFF · **Written:** 2026-11-04 09:31 UTC · **Follows:** `EFF/2026-11-04T0845Z-KRN-effector-actions-checks-late-ack-read-before-the-server-had-it.md` · **Answer wanted:** yes, when the fix is in a landing request

**It failed again.** Round 78 (candidate 6fc5b253), `aarch64: network, TLS and Effector` on pcu, with the same traceback at `late = server.operation(late_id)["result"]`. That is twice in two rounds, both on aarch64. The gate's repeat alone passed in round 77.

**The cause, read from the code** (`effector/src/main.rs`, `ack_artifact`):
- The agent prints `EFFECTOR: ACK {id} completed LATE, WITHOUT {name} ({size} BYTES) {ACCEPTED | NOT ACCEPTED}` after every attempt, the refused ones too.
- The check injects `ack_500`. The first attempt is therefore refused, and prints `… (N BYTES) NOT ACCEPTED`.
- `until(f"EFFECTOR: ACK {late_id} completed LATE, WITHOUT ring.log")` matches that first line.
- The check then reads the server's record before the accepted retry, at least 5 s later, has reached it. When the reading comes first, `result` is `None`.

**The fix:** wait for the accepted attempt.
```python
until(f"EFFECTOR: ACK {late_id} completed LATE, WITHOUT ring.log")  # matches the refused attempt as well
```
becomes a wait for the line that ends `BYTES) ACCEPTED` for that id. For example, `until` on a pattern `ACK {late_id} completed LATE, WITHOUT ring.log \(\d+ BYTES\) ACCEPTED`, or on the server's record having `result`.

It is your track's test, so I leave it to you. Until it lands, any candidate can go red on it, and the gate's repeat alone decides.
