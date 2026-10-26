# 000-NET-0049 — A flow grant asked of the user at run time, and the browsing grant

**Type:** network (`netpolicy`, `idl/netpolicy.wit`) · **Owner:** `NET` · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Main task:** — (for 255, 261 and 000-NET-0048) · **Constitution:** MC-3.1, MC-3.8, MC-10.3, MC-11.6, MC-12.4

Recorded by the network track on 2026-10-25, for the plan [docs/communications-and-agents.md](../docs/communications-and-agents.md). 255 asks for the browsing grant when its stage 1 starts.

## Problem

- **A flow grant today is a line of `netpolicy.txt`:** a program, a host, a port, a term and a volume, written by the owner beforehand.
- **Some programs reach a host the user names only when they run:**
  - `get URL`;
  - `mail` with the user's server;
  - a link in `www`.
- **The browser needs more than one host:** any public address on ports 80 and 443. That is wider than any program holds today, and needs its own term, volume and audit (255).

## Plan

- **`netpolicy.wit`, its next minor version:** `ask(host, port, why)` for the calling program.
  - `netpolicy` puts the question to the user on the console the program was started from, through the shell's question path, as lent clients already ask.
  - The program cannot answer its own question.
  - The answers: no; once (this flow); for this session; always.
  - "Always" adds the line to `netpolicy.txt` through the policy's owner, with a term and a volume.
- **The browsing grant:**
  - a line kind of its own, `browse <program> public tcp 80,443 <term> <volume>`;
  - for named programs only;
  - no private, link-local or loopback address, unless the user names one;
  - an audit line for each new host (MC-10.3).
- **Refusals** are logged with the program, the host and who refused.

## Acceptance criteria

- **Host tests:** the policy's new line kinds, the private ranges a browsing grant refuses.
- **In QEMU, x86:**
  - a test program asks for a host and is refused, then allowed once;
  - it asks again and is asked again;
  - "for this session" holds until the program ends;
  - "always" writes the line;
  - a browsing grant reaches two hosts with an audit line each, and is refused a private address.
- **Docs:** docs/network and the profile's MC-11.6 row.

## Related

[351-NET-0003](../issues-done/351-NET-0003-names-in-the-network-policy.done), [108](../issues-done/108-editable-network-policy.done), [255](255-text-web-browser.md), [000-NET-0048](000-NET-0048-get-a-wget-and-curl-analog.md).
