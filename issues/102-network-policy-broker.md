# 102 — Network policy broker and flow capabilities

**Type:** service · **Owner:** network track · **Priority:** P2 · **Status:** open · **Blocked by:** 101 · **Roadmap:** track D · **Constitution:** MC-11.4, MC-11.6, Appendix B.6 (policy broker)

## Problem

A program must reach only the destinations its purpose needs (MC-11.6). Without a broker, any holder of the stack's endpoint could open any flow.

## Plan

- `netpolicy` service: holds the stack's "open flow" authority and grants flow capabilities for a destination (address or name), port, term, rate and volume according to a policy file.
- Launchers ask the broker on behalf of a program, as they ask for files today; nothing is granted by program name.
- Every grant and refusal goes to the system log.

## Acceptance criteria

- A program with a grant for one host and port reaches it and nothing else; the refusal is logged; revoking the grant ends its flows.

## Related

[101](../issues-done/101-network-stack.done), [103](103-tls-service.md).
