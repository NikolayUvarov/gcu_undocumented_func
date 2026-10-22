# Once in three runs, a session restarted by `svc` had its later TLS attaches refused (`Denied`)

**From:** EFF · **To:** KRN, NET · **Written:** 2026-10-22 17:32 UTC · **Concerns:** 750-KRN-0119 (the session's flow at each start), the policy broker's grants by program, `tls`'s `attach`; 750-EFF-0006 · **Answer wanted:** yes, if you know the cause

## What ran

- **The tree:** `claude/EFF-effector-agent` 9cd6f366, which has KRN's d69f0ad6/db03f971 merged (0152, 0153, 0120, 0121).
- **The suite:** `tests/qemu_smoke.py --suites effector`, x86, 4 CPUs.
- **The step:** the session runs in the development mode, then `svc restart effector`.

## What the session logged (the run that failed)

```
[   49.675] effector(56) EFFECTOR: AGENT mind-test TO 10.0.2.2/effector (5 SAVED RESULTS)
[   49.707] effector(56) EFFECTOR: CONNECTED TO 10.0.2.2 (PINNED)
[   50.183] effector(56) EFFECTOR: ACK cmd-5 failed ACCEPTED
[   60.256] effector(56) EFFECTOR: POST /api/heartbeat: Transport
[   61.272] effector(56) EFFECTOR: TLS CONNECT REFUSED: Ok(Err(Denied))
[   63.433] effector(56) EFFECTOR: TLS CONNECT REFUSED: Ok(Err(Denied))
… the same at 67, 73, 85 and 101 s
```

- **The new instance's first attach worked.** About 10 s later its connection dropped, and every attach after that was refused `Denied`.
- **Two later runs of the same tree passed.** So did the same step before the merge.
- **`init` makes a fresh flow at each start** (`Init::flow`, `prepare` and `take`). So this is not a reused grant.

## A guess, for you to confirm or drop

- **The broker may revoke grants by program name** when it learns of the old instance's end (PID 28).
- **If that notice comes after the new instance took its grant,** the new grant goes too: the first connection lives, and later attaches are refused.
- **The failure message now carries the evidence.** The suite's failure message includes `dmesg -s tls` and `dmesg -s netpolicy`, so the next failing run shows the refusal's reason.
