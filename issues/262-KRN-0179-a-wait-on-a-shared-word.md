# 262-KRN-0179 — A wait and wake on a word of shared memory

**Type:** kernel (ABI: two calls) · **Owner:** `KRN` · **Priority:** P3 · **Status:** open (planned; nothing made) · **Blocked by:** — · **Main task:** [262](262-model-runtime-on-the-machines-full-hardware.md) · **Constitution:** MC-3.1, MC-6.1, MC-12.4

**Asked by:** the tools track's design of `nnd` ([docs/nn/runtime.md](../docs/nn/runtime.md), question 4), with [262-KRN-0178](262-KRN-0178-nnd-as-a-boot-service.md).

## Problem

- **What a task can wait on today.** A task can wait on an endpoint, an interrupt or a time, but not on a word in memory it shares with its workers.
- **What that costs `nnd`.** A worker kept between jobs would have to spin or poll, so `nnd` starts a worker per job and pays the start each time.

## Plan

- **Two calls:**
  - **wait:** the caller sleeps while the word at an address in its own mapping still holds a given value, with a deadline;
  - **wake:** wakes up to N waiters on that word.
- **The word must be in memory the caller has mapped:**
  - its own memory, or a shared object (a lease) both map;
  - the kernel keys a waiter by the object and offset, not by the virtual address, so tasks that map the same object at different places meet.
- **What the kernel guarantees, and what it does not:**
  - no lost wake-up between the check and the sleep;
  - a deadline as for other waits (000-KRN-0065);
  - no ordering among waiters is promised.
- **Accounting:** a waiter costs nothing beyond its task. The number of distinct words with waiters is bounded per task.
- **The ABI:** a new version, with `docs/api` and `libmind` wrappers.

## Acceptance criteria

- **Host tests:** the wait table's keying and bounds.
- **QEMU, x86 and aarch64, 1 and 4 CPUs:**
  - a worker waits on a word of a shared object and a second task wakes it;
  - a wake before the wait is not lost;
  - a deadline returns;
  - a word in memory the caller has not mapped is refused.

## Related

[262-KRN-0178](262-KRN-0178-nnd-as-a-boot-service.md), [262-APP-0112](262-APP-0112-nnd-and-nn-wit-stage-0.md), 000-KRN-0065.
