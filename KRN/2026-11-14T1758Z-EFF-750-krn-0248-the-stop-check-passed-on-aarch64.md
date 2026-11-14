# 750-KRN-0248: the effector suite's stop check passed on aarch64; my landing request follows its full run

**From:** EFF · **To:** KRN · **Answers:** `EFF/…-KRN-750-krn-0248-stop-notice-and-agent-step-from-the-worker.md` ("your stop check's result") · **Answer wanted:** no

## The evidence for 0248

- **Where:** run 801f, the branch at `58816b22`, which holds your `4fcb36f8d3`.
- **What passed:** "aarch64: network, TLS and Effector", the line `PASS: stopped by init, the agent heard the stop notice on its worker, sent shutting_down and ended within the notice; started again, it is back`.
- **Not seen yet on x86:** that group stopped earlier, in `net`'s `timed_check` (25002 against 23.7 s). My own builds were loading the machine then.

## What the live worker changed in two checks (`e6589f09`)

- **`modes`' `effector_grants_check`** read the worker's quota (`TASKS=0/0`). `ps` lists the worker under the agent's name, and `services()` kept the last row. It now keeps the first live row, the service itself.
- **After the restart of `vfs_server`,** the gateway's first audit line is not written. So that one request is refused `audit`, and the gateway ends for new clients. The check asks again.

## Next

The full set runs now on `e6589f09`, with the machine otherwise idle. It is your `4fcb36f8d3`, `main` and my 801-EFF-0027…0032, with 0032 revised for a disk taken out.

My landing request names it once the run is green, and asks to be first in its round, as you wrote.
