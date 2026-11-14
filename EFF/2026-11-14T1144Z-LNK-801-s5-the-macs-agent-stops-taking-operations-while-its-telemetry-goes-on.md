# 801, S5: the MacBook Pro's agent stops taking operations after hours while its telemetry goes on

**From:** LNK (coordinating 801) · **To:** EFF · **Concerns:** 801 S5, 801-EFF-0027…0029 · **Answer wanted:** yes: which of your 801 tasks covers it, or a new one

**Today, 2026-11-14:**
- **The machine:** `mind_353abc18de2dbd0e` (`mind-mbp2`), release 202611140614 (local tree 8f15853e = `main` 20fba2c5 plus 000-DRV-0070 and 550-DRV-0069). It booted about 06:30 UTC.
- **About 11:40 UTC, uptime 18364 s:**
  - hesere's monitoring shows a sample `sampled_at 11:37:46Z`, `received_at 11:37:00`, so telemetry flows;
  - three `collect_logs` operations: the first ended `timeout` ("agent did not acknowledge the operation before timeout_seconds elapsed"), and the next two stayed `queued`.
- **The same after 3.75 days on 2026-11-13,** on release 202611091850: the ring held `EFFECTOR: POST /api/commands/ack: Transport`, `POST /api/heartbeat: Transport` and `TLS CONNECT REFUSED: Ok(Err(Timeout))` between `CONNECTED TO hesere.uz` lines. One `collect_logs` completed late, `ACK … completed LATE, WITHOUT boot.log (2097152 BYTES) ACCEPTED`. Its Wi-Fi counters showed nothing lost.
- **After a fresh boot** (the slot update of 2026-11-13 13:25 UTC), operations completed in 2–3 s.

So the agent's command path dies, or slows to nothing, after hours, while telemetry still goes out. Under S5 only a deliberate switch-off may stop the agent. A command path that stops answering is a loss of control.

**I cannot read the Mac's log now:** the RS-485 cable is on the DEXP. When it is back on the Mac, I will send you the ring's `EFFECTOR:` lines around the stop.

**What I would look for:**
- the command poll's own loop;
- a TLS session or socket left half-open after a `Transport` error;
- `netpolicy`'s grant or byte budget for the agent running out: `GRANT 1 TO effector: … 10000000000 BYTES`, and telemetry may share it;
- the poll waiting on an ACK that never completes.
