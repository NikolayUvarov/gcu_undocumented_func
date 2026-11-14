# 801-NET-0080 — The server's `Date` in an HTTP head, for a client that judges by the server's time

**Type:** network (`libmind` `http` and `parse`, `parse.wit` 5.0) · **Owner:** `NET` (the kernel session) · **Priority:** P1 · **Status:** open (2026-11-14) · **Blocked by:** — · **Main task:** [801](801-the-bench-stays-manageable.md) (S5) · **Roadmap:** stage V · **Constitution:** MC-7.5, MC-11.11, MC-12.4

## Problem

**A clock wrong by hours stopped the agent.** On the DEXP, whose RTC keeps Windows' local time (+3 h), every command was `command_expired`. EFF's agent will judge a command's expiry by the server's time (801-EFF-0027, S5). It keeps an offset from the `Date` header of each answer it accepts.

**`mind::http::Head` drops `Date`.** A response head is parsed by the parser service, not by the program that holds the network (MC-11.11), and `parse.wit`'s `http-response` does not carry the field. EFF asked for it (`KRN/2026-11-14T0909Z-EFF-801-net-the-servers-date-for-the-agent-and-tls-with-a-wrong-clock.md`, request 1).

## Plan

- **`Head.date: Option<u64>`:** a valid `Date` (RFC 9110 HTTP-date, in any of the three forms `http_date` reads) as Unix seconds. An invalid one is ignored, as an invalid `Retry-After` is.
- **`parse.wit` 5.0.** `http-response` gains `has-date` and `date`. The record's encoding changes, so the version is major; 4.x's response is gone (MC-12.4, the rule of 2026-10-25).
- **The offset itself is EFF's**, in the agent.

## Acceptance criteria

- **The host test** `heads_say_the_servers_date`: the three forms are read to the same second, and invalid dates are none.
- **The groups that use the parser** (both arches' network, TLS and Effector groups) pass on the maintainer's machines, then in a gate.

## Related

[801](801-the-bench-stays-manageable.md), [000-NET-0047](../issues-done/000-NET-0047-http-for-api-clients.done) (`Retry-After` and the other fields for API clients).
