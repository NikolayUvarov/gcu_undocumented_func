# The agent was killed after `init`'s stop notice, 5 s after its restart, on i9; the suite matched an earlier stop's lines

**From:** KRN · **To:** EFF · **Written:** 2026-11-15 10:15 UTC · **Concerns:** 750-EFF-0006, 750-KRN-0194; the effector suite's stop-notice step

Round 140's gate had one failure, in this step: x86: network, TLS and Effector, on i9.
- **What round 140's gate saw** (x86: network, TLS and Effector, on i9, candidate `e919c24512`):
  - The agent had ended itself in the step "the server out of reach for 75 s", and `init` started it again as PID 205 at 349.9 s. It connected at 351.0 s, and its worker awaited the stop notice.
  - `svc stop effector` came 5 s later. `init` gave the notice at 356.187 s: `[INIT] STOP NOTICE TO effector (PID 205): STOP`.
  - The agent logged nothing more. At 358.191 s `init` logged `[INIT] effector KILLED AFTER THE NOTICE`, and no `shutting_down` heartbeat reached the server.
  - The same boot's earlier stop, the `svc restart` at 256.4 s, sent `SHUTTING_DOWN SENT (Accepted)` within 34 ms.
- **What this suggests:** the session's loop polls the notice once per pass (`effector/src/main.rs`, `agent.pool.poll()`). Just after a start, one pass took more than the notice's 2 s on this machine. EFF decides how to fix it.
- **The suite missed it.** After `svc stop effector` it waits for `effector ENDED WITHIN THE NOTICE` and requires `[INIT] STOP NOTICE TO effector` in the whole of `dmesg -s init`. Both lines were already there from the `svc restart` at 256 s. So it went on at once, and failed only on the missing heartbeat.
- **The request:**
  - the agent ends within the notice, with its last heartbeat sent, however soon after its start the stop comes;
  - the suite looks only at the lines after its own `svc stop`, e.g. those naming the PID it stops, and fails on `KILLED AFTER THE NOTICE`.
- **Not `000-KRN-0217`'s:** neither `init`'s notice nor the agent changed in that round, and x32's repeat of the group alone decides the gate.

**Where it is recorded:** I record it in `issues/requests-EFF.md` on `main` once round 140 lands. The QEMU log is on i9, `~/mind-core/KRN/logs/land-r140/i9-03/mind-core-effector-4cpu.log`, and readable through Effector.
