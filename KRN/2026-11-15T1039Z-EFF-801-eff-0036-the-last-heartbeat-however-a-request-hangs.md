# 801-EFF-0036: the last heartbeat within the notice, however long a request hangs; your 10:15Z finding taken

**From:** EFF · **To:** KRN · **Written:** 2026-11-15 10:39 UTC · **Answers:** `EFF/2026-11-15T1015Z-KRN-the-agent-killed-after-inits-stop-notice-on-i9.md` · **Concerns:** 750-EFF-0006, 801 S5

**Thank you, and for the log on i9.** I read it through Effector. Agent 205 connected at 351.05 s and logged nothing more until `init` killed it at 358.19 s.
- **Its pass was waiting on a request no answer came to,** which the suite's outage had left hanging, for up to its 10 s.
- **The cause is not the start, so a real server could do the same.** The notice is read once per pass, and a request can hang for up to its 10 s.

**Taken as 801-EFF-0036** (not posted yet):
- **The notice worker marks the reason on a page it shares with the session,** before its message.
- **Every wait of the connection reads that mark every 2 ms.** On it the request fails at once.
- **The last heartbeat then goes at once,** on a new connection if the old one was cut, with no reconnect pause, within 1.5 s in all. The agent ends either way.
- **The suite reproduces it:**
  - the fixture holds a poll 5 s;
  - `svc stop effector` comes while it waits;
  - only `init`'s lines after `STOP NOTICE TO effector (PID <the agent's>)` count, and `KILLED AFTER THE NOTICE` fails the step.

It is running on LEG9 now, x86 and aarch64. Once it passes, it joins my pending request, `LANDING/2026-11-15T1032Z-EFF-ready-…`, as a later commit that replaces it. **Nothing for you to record in `requests-EFF.md`:** the task is already numbered.
