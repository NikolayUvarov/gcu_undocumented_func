# 111-NET-0076 — The refused NTS servers' check waits for each server's key establishment

**Type:** network (a test of `timed`) · **Owner:** `NET` · **Priority:** P2 · **Status:** in progress · **Blocked by:** — · **Main task:** [111](111-network-time.md) · **Roadmap:** track D · **Constitution:** MC-12.2

## Problem

Round 129's gate (2026-11-13) failed "x86: network, TLS and Effector" on i9 (1239 s), in the `tls` suite's NTS check of refused servers:
- **The check:** the second boot names three servers, one agreeing on no application protocol, one with a rogue certificate and one honest. At timed's first `NOTHING SET` it looks for each server's key establishment line.
- **What the log showed:** the rogue server refused (`Certificate`) and the honest one's 8 cookies, but no line for the first server. timed's round counted one server: `NOTHING SET: FEWER THAN 2 ANSWERS (1 OF 1 ANSWERED)`.
- **Why:** netpolicy made timed's grant before the network was up. The first server's two rules waited for their name (`mind.test WAITS FOR THE NETWORK`) and joined the grant at 2.4 s, after timed's first round (`2 RULES ADDED, ITS HOST NAMES RESOLVED ONCE THE NETWORK CAME UP`). Its key establishment comes in a later round. This is 111-NET-0074's behaviour, which the first boot of the check allows for and the second did not. Read from the run's logs (`mind-core-nts-refused-4cpu.log`, `mind-core-nts-4cpu.log` on i9).
- **Where it shows:** a machine where netpolicy's grant comes before DHCP. The group passed in rounds 127 and 128 elsewhere; i9 had been out of the gates since round 113.

## Plan

The second boot waits until timed's lines hold each of the three servers' key establishment, up to 180 s, then checks them as before.

## Acceptance criteria

- The `tls` suite passes on x86 and aarch64, on i9 among the machines.

## Progress

**Made (2026-11-13)** on `claude/111-NET-0076-nts-refused-race`: `tests/qemu_smoke.py` `nts_check`.
- **Not run in this container:** its `tls` suite stops before the NTS check, at Python's `cryptography` there (`No module named '_cffi_backend'`). It runs on the maintainer's machines, i9 among them.

## Related

[111-NET-0074](../issues-done/111-NET-0074-a-grants-host-names.done), [111-NET-0075](../issues-done/111-NET-0075-timed-with-nts.done).
