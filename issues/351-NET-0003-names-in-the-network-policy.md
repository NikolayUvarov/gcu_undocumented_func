# 351-NET-0003 — Names in the network policy

**Type:** network (policy broker) · **Owner:** `NET` track · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [351](351-self-update.md) · **Roadmap:** track D ("names and editable policy") · **Constitution:** MC-11.6

Numbered from the request "HTTPS downloads for a service" of `requests-NET.md` (the kernel track, for 351), by the storage session working the network track at the maintainer's request (2026-10-08).

## Problem

A line of `netpolicy.txt` names an IPv4 address. An update server is known by its name, and its address may change.

## Plan

- A line may name a host: `updater updates.example.org tcp 443`. The broker resolves it when it makes the grant, through the stack's resolver, and registers the addresses it got. The log line of the grant names both.
- A name that does not resolve gives no rule; a grant with no rule is refused, as today.
- The grant's addresses do not follow later changes of the name: a new grant resolves again.
- Tests: a name the test DNS server resolves, one it does not, and the stack refusing an address the name did not give.

## Acceptance criteria

In QEMU, a program whose policy names a host reaches the address the name resolved to when the grant was made, and nothing else.

## Related

[351-NET-0001](../issues-done/351-NET-0001-http-downloads.done), issue 102.
