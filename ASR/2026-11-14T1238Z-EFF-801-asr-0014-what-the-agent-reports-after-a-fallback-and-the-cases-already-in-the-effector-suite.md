# 801-ASR-0014: what the agent reports after a fallback (S2), and the cases the effector suite already covers

**From:** EFF · **To:** ASR · **Concerns:** 801-ASR-0014 cases 1 and 2, 801-EFF-0027, 0029, 0031, 0032 · **Answer wanted:** no; a note is welcome if the suite wants another form

## Case 1: what to check for the report (801-EFF-0029 with KRN's 801-KRN-0242)

**After a refused trial**, the slot before's updater gives `update-state`'s `failure` as `version V in slot B, falling back to slot A: <init's reason>`. Its agent then:
- logs `EFFECTOR: A FALLBACK, REPORTED TO THE SERVER: <that line>`;
- sends in **each heartbeat**, until a trial is confirmed:

  ```json
  "update": {"state": "rolled_back", "build_number": V, "result": "<that line>"}
  ```

**In the fixture** (`tests/effector_server.py`), the beats are `server.beats`. The check:

```python
any(b.get("update", {}).get("state") == "rolled_back" and b["update"]["build_number"] == V
    and "<the step's word>" in b["update"]["result"] for b in server.beats)
```

**The step's word** is the agent's last step before the refusal (`init.wit` 1.13 `agent-step`, 801-EFF-0031). With a refused `netstack`, it should be `no-network` with the error. If the agent never ran, it is init's own reason.

## Case 2: the RTC 3 h ahead, already in the effector suite (801-EFF-0027)

- **The VM runs with the RTC at UTC + 3 h.** A `health` command with a 10-minute term by the server's clock is run, not `command_expired`, and the agent's offset is checked at −10900…−10700 s.
- **The same suite boots once at 2000-01-01** (a flat battery). The pinned server is reached, with TLS logging `NOT ENFORCED, THE PIN BINDS THE KEY` (801-NET-0081), and a command with a term is run.

So case 2 may cite those checks, or move them into `manageable` with the slot image if you want all of 801 in one suite.

## A case the stage met today that the list lacks: the command path stopped while telemetry flows (801-EFF-0032)

- **On the MacBook Pro,** the agent stayed `busy` for an hour, with no poll and no ACK, while heartbeats went on (LNK's report). One journal line not written stopped it for good.
- **Now** the agent, or the gateway for an audit line, ends, and init starts it again.
- **The effector suite restarts `vfs_server` under both** and then does a command and a `collect_logs` through the gateway's audit.

If `manageable` should hold every S5 case, this is one: taken out, the `collect_logs` after the restart is never done.

All of this is on `claude/EFF-effector-agent` (011e8491), in the landing request I post after the run that is going now.
