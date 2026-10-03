# 030 — IPC bounds, timeouts and cancellation (roadmap C4, part 3)

**Type:** architecture · **Priority:** P1 · **Status:** open · **Roadmap:** step 3, C4 · **Constitution:** MC-2.5, MC-2.11, MC-2.13

## Problem

A send or call waits without limit: a client of a server that never replies waits forever, and the number of senders waiting on one endpoint is bounded only by the task count. There is no way to give up a request.

## Plan

- Each endpoint has a bounded wait queue (`ENDPOINT_QUEUE` senders); a send beyond it fails with `ERR_BUSY` (back-pressure) instead of blocking.
- `IPC_SEND`, `IPC_CALL` and `IPC_RECV` take an optional deadline (monotonic ns, 0 = none); on expiry the operation fails with `ERR_TIMEOUT` and leaves no trace: a waiting sender is removed from the queue, a pending capability returns to the sender.
- A call whose reply is no longer awaited (the client timed out or died) makes the server's later `IPC_REPLY` fail with `ERR_PEER`.

## Acceptance criteria

- Isolation or services fixture: a send past the queue bound fails with `ERR_BUSY`; a call to a server that never replies returns `ERR_TIMEOUT` at the deadline; a late reply fails with `ERR_PEER`.
- All QEMU suites pass.

## Related

[028](../issues-done/028-memory-rights-and-lease.done), [029](029-memory-objects-move-seal.md), [ROADMAP](../ROADMAP.md) C4.
