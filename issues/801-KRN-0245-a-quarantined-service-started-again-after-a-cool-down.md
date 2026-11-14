# 801-KRN-0245 — A quarantined service started again after a cool-down, so none on the path to the server stays down for good

**Type:** kernel (`init`) · **Owner:** `KRN` · **Priority:** P1 · **Status:** open (2026-11-14) · **Blocked by:** — · **Main task:** [801](801-the-bench-stays-manageable.md) (S5) · **Roadmap:** stage V · **Constitution:** MC-6.5, MC-6.8, MC-9.9

## Problem

**A quarantine is for good.** `init` restarts a failed service at most 3 times in 60 s (MC-6.5), then quarantines it until someone runs it again.
- On the path to the Effector server, that someone is out of reach: the network driver, `wlan`, `netstack`, `netpolicy`, `tls`, `parse`, `effector_gw` and `effector` itself.
- A burst of faults, a radio that resets three times in a minute or a server that answers badly, leaves the machine off the network until a hand at it or a power cycle.

The maintainer (801 S5): only a deliberate switch-off stops the agent.

## Plan

- **A quarantine by the restart budget has a cool-down.** It is 1 min at first and doubles at each quarantine, up to 30 min. Past it, `init` starts the service again with a fresh budget: `[INIT] <name> STARTED AGAIN PID=… AFTER … S IN QUARANTINE`.
  - **A start that fails** waits the next, longer cool-down.
  - **The quarantine's line names the cool-down:** `… QUARANTINED: 3 RESTARTS IN 60 S; STARTED AGAIN IN 60 S`.
- **MC-6.5's bound holds:** one start a cool-down, so a service that always fails costs one start every 30 min at most.
- **Not started again:**
  - a service stopped on request;
  - one quarantined because its device's DMA could not be stopped (MC-6.3), which stays until `RUN`.
- **A trial still refuses** at the agent's first quarantine (801-KRN-0242).
- **`RUN` lifts a quarantine at once,** as before, and its cool-down starts again from 1 min.

## Acceptance criteria

- **The `services` suite:**
  - `rtc`, quarantined after its fourth death, is started again by `init` 60 s later, and its client reaches it;
  - quarantined once more, its cool-down is 120 s;
  - `RUN` lifts that at once.
- **The groups that run `services`** pass on both arches on the maintainer's machines, then in a gate.

## Related

[801](801-the-bench-stays-manageable.md), [801-KRN-0242](801-KRN-0242-a-trial-confirmed-on-the-effector-servers-word.md).
