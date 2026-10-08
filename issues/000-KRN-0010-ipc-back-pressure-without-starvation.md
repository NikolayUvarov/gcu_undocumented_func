# 000-KRN-0010 — IPC back-pressure without starvation

**Type:** kernel · **Owner:** `KRN` · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Roadmap:** stage II (bounded queues and progress) · **Constitution:** MC-2.10, MC-5.2

## Problem

At most `ENDPOINT_QUEUE` (4) senders wait on an endpoint. Beyond that, a send gets `ERR_BUSY`, and `libmind::ipc` waits 10 ms and tries again within the caller's timeout. The retries do not keep order: a client that has waited longest has no better chance than one that just came.

[171-KRN-0009](../issues-done/171-KRN-0009-sixteen-cpus-at-the-peak.done) measured it. On aarch64 with 16 CPUs and 120 clocks, each clock asks the RTC service for the time 10 times a second. A `date` from the shell then took 8.5 s to get one of the four places. The retries are themselves system calls under the scheduler lock. With 60 clocks on x86, `UPTIME` alone was called 1 500 times a second, mostly by these loops.

MC-5.2 asks that budget exhaustion lead to a defined outcome, such as refusal or bounded waiting, and that overload not be compensated for by unbounded accumulation. MC-2.10 asks for progress conditions on protocols. The queue is bounded, but a client's wait for a place is not.

## Plan (to be decided)

- **Measure first.** Count retries per send and the longest wait for a place, per endpoint, in `STAT_ENDPOINTS` (it has `busy` already).
- **Options:**
  - Senders beyond the queue wait in order in the kernel, bounded by the sender's own task quota, so the queue stays bounded by the tasks that exist rather than by 4.
  - Or keep `ERR_BUSY` and give each endpoint a ticket order that the retry presents.
  - Either way the outcome is defined: refusal at the caller's timeout, or service in order.
- **Interface.** `ENDPOINT_QUEUE` and `ERR_BUSY` are in `common/abi.rs`. A change to their meaning is an ABI change and takes a new ABI version (MC-12.4).

## Acceptance criteria

- With 120 clocks on aarch64 and 16 CPUs, a `date` from the shell is answered within a bound stated in `docs/profile`.
- A test shows a waiting client served before later ones.

## Related

[171-KRN-0009](../issues-done/171-KRN-0009-sixteen-cpus-at-the-peak.done), `requests-APP.md` (the clocks' polling).
