# 750-NET-0014 — The bounds of a flow, for a long-running service

**Type:** network (policy) · **Owner:** `NET` track (worked by the storage session at the maintainer's request) · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [750](750-effector-agent.md) (the network track's part) · **Roadmap:** track D · **Constitution:** MC-11.6, MC-12.3

From the Effector agent track's request in requests-NET.md ("A flow for a long-running service", 2026-10-09).

## Problem

`init` gives a boot service its flow once, and a grant keeps its term and volume until it ends: by default 3600 s and 16 MiB. The Effector agent talks to one server for as long as the machine runs, and moves packages and files of up to 1 GiB. The policy guide does not say how long a term or how large a volume a line may give. A line naming the largest term its number holds (4294967295 s) or the largest volume (2^64 − 1 bytes) got the default instead, since `netpolicy` used those values to mean "none named".

## Plan

- **`netpolicy`:** a line's term and volume are kept as named, up to the largest values their numbers hold.
- **The policy guide** states the bounds: what a term and a volume count, what ends a grant, how a long-running service's line looks, and how `left-ms` shows a term longer than 49.7 days.
- **Later, if the track agrees** (not in this task):
  - a renewable flow;
  - a volume per period;
  - a name looked up again on reconnecting.

  Any change to how `init` hands out the flow is a kernel task.

## Acceptance criteria

- The policy guide states the bounds.
- In the `net` suite (x86, aarch64):
  - a line with the largest term and volume gives a grant with them;
  - a grant's short term ends the connection it holds, while a grant with a long term keeps its connection.
- A connection held past the default term (an hour) is not run in QEMU; the guide says so.

## Related

[docs/network/policy.md](../docs/network/policy.md), [102](../issues-done/102-network-policy-broker.done), [108](../issues-done/108-editable-network-policy.done).
