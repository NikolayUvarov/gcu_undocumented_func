# 032 — Minimal supervision (roadmap C6)

**Type:** architecture · **Priority:** P1 · **Status:** open · **Roadmap:** step 3, C6 · **Constitution:** MC-6.2, MC-6.4, MC-6.5, MC-6.8, MC-6.9, MC-6.12

## Problem

Nobody learns that a service died: `init` restarts one only when the operator types `RUN <service> &`. There is no restart budget. Senders queued on a dead server's endpoint are delivered to the next instance as if nothing happened (MC-6.4). If `init` itself dies, the system keeps running without its bootstrap authority and policy.

## Plan

- **Failure notification (kernel mechanism):** `TASK_WATCH(pid, endpoint)`. Only the task's lifecycle owner (its spawner) may register an endpoint it can receive on. When the task ends, the kernel queues a notice: a message with `MSG_FLAG_EXIT`, `data = [pid, reason]`, where the reason is exit, fault (with the vector) or kill. Notices are bounded (16); the oldest is never dropped silently, the count of lost notices is reported.
- **Old instance (MC-6.4):** when the last task that can receive on an endpoint ends, every sender still queued on it fails with `ERR_PEER`. A message addressed to a dead instance is never delivered to the next one. Clients tell instances apart by the server PID in replies (`Received.sender`).
- **Supervision policy (`init`):**
  - `init` watches every service it starts and restarts a failed one automatically.
  - Budget: 3 restarts per 60 s per service. When it is spent, the service is quarantined (`[INIT] <name> QUARANTINED`) and not restarted again automatically. An explicit `RUN <service> &` from the operator restarts it and resets the budget.
  - `init` restarts services from its own reserved quota; the old instance's quota returns before the restart.
- **Final recovery boundary (MC-6.8):** if `init` ends, the kernel logs `INIT EXITED` and halts all CPUs. No unsupervised state is left running.

## Acceptance criteria

- Services suite: a killed service comes back by itself and serves its existing clients again. After the budget is spent, it is quarantined and stays down until `RUN`.
- A sender queued on a dead server fails with `ERR_PEER` instead of reaching the new instance.
- All QEMU suites pass.

## Related

[027](../issues-done/027-no-ambient-endpoint-names.done), [030](../issues-done/030-ipc-bounds-and-cancellation.done), [ROADMAP](../ROADMAP.md) C6.
