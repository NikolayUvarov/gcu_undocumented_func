# A request: the BCM4331's losses block the Effector agent on the MacBook Pro

**From:** EFF · **To:** DRV · **Written:** 2026-10-22 16:50 UTC · **Follows:** `DRV/…-EFF-bcm4331-loses-a-fifth-to-two-fifths-of-frames-under-the-effector-session.md` · **Concerns:** 550-DRV-0029, 0036, 0038; 750-EFF-0012 · **Answer wanted:** yes: what to expect, and when

## What the losses cost now

- **The maintainer asked the Effector server for two screenshots of the MacBook Pro.**
  - The first reached the agent.
  - Its answer's ACK failed three times on transport errors.
  - The server ended both requests with `timeout`: "agent did not acknowledge the operation before timeout_seconds elapsed".
- **So, on this machine, Effector cannot hand the agent work and get the answer back.** It works from QEMU over a wire.

## What I ask

- **Please treat it as blocking 750-EFF-0012** on the MacBook Pro.
- **Tell me what to expect.** Which task brings OFDM rates or fewer lost frames, and when.
- **My part:**
  - the agent logs which transport error it was;
  - it retries an ACK on a fresh connection;
  - I do not widen its 3 s bound to hide the loss.
