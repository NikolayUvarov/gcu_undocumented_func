# 801-NET-0081 — A pinned server reached with a clock far behind: a certificate's start after the clock is logged, not refused

**Type:** network (`tls`, `tls.wit` 1.5) · **Owner:** `NET` (the kernel session) · **Priority:** P1 · **Status:** open (2026-11-14) · **Blocked by:** — · **Main task:** [801](801-the-bench-stays-manageable.md) (S5) · **Roadmap:** stage V · **Constitution:** MC-11.5, MC-12.4

## Problem

**A flat battery stops the agent.** `connect-pins` (750-NET-0020) checks the leaf's validity period against the system clock. With a flat battery the RTC reads, for example, 2000, and the pinned Effector server is `NotValidYet`. The agent never connects, so it cannot learn the server's time either. EFF asked for this (`KRN/2026-11-14T0909Z-EFF-801-net-the-servers-date-for-the-agent-and-tls-with-a-wrong-clock.md`, request 2).

## Plan

- **A start after the clock is logged, not refused.** For a server known by its pin, a clock before the certificate's start is taken as a wrong clock: a flat battery or an unset RTC.
  - The pin binds the key, so the start is logged and not enforced: `[TLS] PINNED SERVER …: ITS CERTIFICATE STARTS AT … BUT THE CLOCK READS …: NOT ENFORCED, THE PIN BINDS THE KEY`.
  - Its end is still checked: an expired certificate is refused, as before.
- **`tls.wit` 1.5** says so. It is a change of what `connect-pins` checks, so it is versioned (MC-12.4).
- **Not affected:** `connect` with the root store, and `connect-pinned` (1.1), which never checked dates.

## Acceptance criteria

- **The `tls` suite's pinned runs:** a server whose certificate starts in 2090 is reached through its leaf pin, and the start is logged. The expired, the wrong-name and the unpinned servers are still refused.
- **Both arches' "network, TLS and Effector" groups** pass on the maintainer's machines, then in a gate.
- **EFF's evidence** (an RTC at 2000-01-01, the agent connects) is its own suite's, once its offset is in.

## Related

[801](801-the-bench-stays-manageable.md), [801-NET-0080](801-NET-0080-the-servers-date-in-a-head.md), [docs/network/services.md](../docs/network/services.md).
