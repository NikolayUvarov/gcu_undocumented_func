# 000-NET-0049 — A flow grant asked of the user at run time, and the browsing grant

**Type:** network (`netpolicy`, `idl/netpolicy.wit`) · **Owner:** `NET` · **Priority:** P1 · **Status:** open (the browsing grant done; asking at run time left) · **Blocked by:** for the user's answers, the shell's part ([requests-APP.md](requests-APP.md), "The shell puts a running program's question about a flow to the user", 2026-10-31) · **Main task:** — (for 255, 261 and 000-NET-0048) · **Constitution:** MC-3.1, MC-3.8, MC-10.3, MC-11.6, MC-12.4

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

## Progress (2026-10-28)

**The browsing grant is made.** Asking at run time is left.

- **`idl/socket.wit` 2.7:** a rule of address 0 reaches any public IPv4 address on its port, TCP only and never port 0.
  - `mind::network::public` says which addresses are public: not 0/8, the private ranges, shared space, loopback, link-local, the IETF's, documentation and benchmarking ranges, multicast or the reserved block.
  - An address a line names keeps reaching what it names.
- **`netstack`:**
  - refuses an address-0 rule for UDP, ICMP or port 0;
  - logs each new host a grant reaches through such a rule, once per host: the last 256 kept, `[NETSTACK] GRANT n BROWSES A NEW PUBLIC HOST A.B.C.D:PORT (k SO FAR)`.
- **`netpolicy`:**
  - reads `browse <program> public tcp <port>[,<port>…] [<seconds> [<bytes>]]` (at most 4 ports; `mind::network::browse_line`), one rule per port;
  - logs such a grant as `BROWSING ANY PUBLIC ADDRESS`;
  - accepts the line in `netpolicy add`.
- **Docs:** `docs/network/policy.md` 0.5, the profile's MC-11.6 row, `docs/idl`.
- **Host tests** (`tests/netpolicy_host.rs`): which addresses are public, at every range's edges; the line read and refused.
- **QEMU, x86, the `net` suite** (`browse_check`). The test network is QEMU's user network on 11.0.2.0/24 with `restrict=on`, so nothing leaves the host. Two of its addresses are forwarded to `cat`.
  - `netcheck`, with only `browse netcheck public tcp 80`, reached both, and logged each host once.
  - It was refused port 81, 10.0.2.2 and 192.168.1.1.

**Left:**
- `ask(host, port, why)` and the user's answers (no, once, this session, always). These need the shell's question path, `APP`'s part.
- The browsing grant with the browser (255) itself.

## Acceptance criteria

- **Host tests:** the policy's new line kinds, the private ranges a browsing grant refuses.
- **In QEMU, x86:**
  - a test program asks for a host and is refused, then allowed once;
  - it asks again and is asked again;
  - "for this session" holds until the program ends;
  - "always" writes the line;
  - a browsing grant reaches two hosts with an audit line each, and is refused a private address.
- **Docs:** docs/network and the profile's MC-11.6 row.

## Progress (2026-10-29)

In `main` at ef3faf0d (landing round 41, the request's commit b4759726): `socket.wit` 2.7, `netpolicy`'s `browse` line, `netstack`'s log of each new host; the `net` suite's `browse_check` passed in the round 41 gate on x86 and aarch64.

Still left: `ask(host, port, why)` and the user's answers, which need the shell's part from `APP`, and the browsing grant with the browser (255).

## Progress (2026-10-31): the split with the shell

- **The shell's part is asked for** in requests-APP.md, as nothing had recorded it:
  - the shell takes the questions from `netpolicy` and puts each to the user on the asking program's console;
  - it sends the answer back: no, once, session or always.
- **The network track's part, next:**
  - `netpolicy.wit` 1.3: `questions` and `answer` for the shell's client; refused for a launcher's;
  - a program's way to ask through its grant;
  - each answer applied: `once` and `session` as rules of the program's live grant, `always` as a line of the policy;
  - a question no one answers in 60 s refused.

## Related

[351-NET-0003](../issues-done/351-NET-0003-names-in-the-network-policy.done), [108](../issues-done/108-editable-network-policy.done), [255](255-text-web-browser.md), [000-NET-0048](000-NET-0048-get-a-wget-and-curl-analog.md).
