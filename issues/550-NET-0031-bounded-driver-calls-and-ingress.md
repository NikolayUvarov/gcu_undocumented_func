# 550-NET-0031 — Bound every card and radio call, and each ingress turn

**Type:** network reliability · **Owner:** ASTRA auditor-fixer, for `NET` · **Priority:** P1 · **Status:** in progress · **Blocked by:** — · **Main task:** [550](550-network-on-real-hardware.md) · **Roadmap:** track D · **Constitution:** MC-5.2, MC-6.1, MC-12.1, MC-12.3

## Problem

The audit of `main` 94ec83a0 and the maintainer's tested build e6730d7a found gaps in [550-NET-0030](550-NET-0030-the-stack-and-the-station-do-not-wait-for-ever.md):

- `netstack` bounds its first `Stack::poll` call, but calls it again after handling a client without the timeout scope.
- `wlan` bounds its periodic pump, but sends to the radio from client `send`, `scan`, `join` and `leave` handlers outside that scope.
- A fresh timeout for each call does not bound the sum of calls to one device.
- `smoltcp::Interface::poll` drains all ingress before returning; a continuously replenished receive queue can keep the service from answering clients or polling its other cards.

The MacBook log from the tested build establishes a successful join and DHCP lease after the explicitly requested radio diagnostic. It does not record the reported later ping failure under load. These are confirmed code defects, not a claim that the hardware symptom has been reproduced or explained. The Ryzen's unsupported Realtek NICs are a separate driver task.

## Plan

- Give each card's turn one deadline; every outbound driver call uses only its remaining time, and an exhausted turn makes no further calls.
- Bound ingress work independently of whether a driver answers quickly; continue timers, egress, other cards and client handling between bursts.
- Apply the same remaining-time rule to every radio operation, including operations reached from client requests.
- Exercise timeout exhaustion, a slow sequence of operations, a continuously nonempty receive queue, and normal network progress with host tests; build the affected services and run the available network QEMU checks on ASTRA.
- Leave the hardware conclusion open until a new run records loss, latency and recovery while the reported load runs.

## Acceptance criteria

- A silent driver cannot cause unbounded IPC through either polling path or a station client handler.
- Calls within a device turn share a deadline instead of accumulating fresh two-second waits.
- Continuous ingress yields after a bounded burst, allowing egress and subsequent service work.
- Tests exercise the production deadline and ingress code; normal network checks continue to pass.
- The report distinguishes host/QEMU evidence from the outstanding MacBook and Ryzen hardware checks.

## Related

[550-NET-0030](550-NET-0030-the-stack-and-the-station-do-not-wait-for-ever.md), [550](550-network-on-real-hardware.md), [213](213-amd-ryzen-pc.md).

## Progress (2026-10-15)

Implemented on ASTRA: `libmind::deadline::Deadline` supplies the remaining timeout, and skips calls after expiry (zero is never passed as an unlimited IPC timeout). All card operations use it, including those reached after a client request. WLAN applies one turn's deadline to its radio and key-store calls, both during pumping and client dispatch. The stack processes at most 64 incoming packets per card per turn, followed by one egress pass. A malformed shared-ring slot no longer leads to an unbounded skip loop.

Only an actually attempted IPC that returns `ERR_TIMEOUT` starts the card's five-second cooldown. Exhausting the work budget while processing successful packets yields normally and does not label the card unresponsive.

The four tests in `tests/network_budget_host.rs` pass with the production deadline and smoltcp polling helper. They cover a sequence of slow calls sharing two seconds, a silent peer receiving only the remaining time, expiry without invoking the peer, recovery on a later turn, continuous ingress with egress and a second card making progress, and ordinary budget exhaustion without an attempted peer timeout. `netstack` and `wlan` release builds for `x86_64-unknown-none` pass. Integrated QEMU and a new hardware run remain pending.
