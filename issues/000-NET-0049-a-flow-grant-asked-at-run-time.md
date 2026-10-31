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
- **The way through, as designed (2026-10-31).** A program holds only its grant, a client of the stack. So the question goes through the stack, and the stack decides nothing.
  - **`socket.wit` 2.8, for a grant's badge:**
    - `ask(host, port, why)`: the host is a name or a dotted address, up to 253 bytes; the reason up to 120. The stack keeps it, at most one a grant and 8 in all, and answers its id at once, as its other calls that do not wait;
    - `asked(id)`: `pending`, `allowed` with the address to reach, or `refused`.
  - **`socket.wit` 2.8, for the policy badge:**
    - `questions()`: what is pending, with each one's grant badge;
    - `decide(id, address, allowed)`: the answer for the program.
  - **`netpolicy`:**
    - takes the questions from the stack in its loop;
    - names each grant's program and PID from its own table;
    - serves them to the shell (`netpolicy.wit` 1.3);
    - on an answer: resolves the host, adds the rule to the grant (`policy-set`), with `always` adds the line as `add` does, and tells the stack.
  - **`once`** is the rule until the program's first connection to that host and port ends. The stack drops a rule marked so when that socket closes (the rule's kind, `socket.wit` 2.8).

## Progress (2026-10-31): the stack's and the broker's part

- **APP's answer** (`NET/2026-10-31T0650Z-APP-…`):
  - the poll suits the shell; it asks `questions()` at most every 500 ms;
  - the program's name is the grant's, the reason is bounded, a question of an ended program leaves the list and `answer` gives it `not-found`, and each question has its PID;
  - its part is [000-APP-0120](000-APP-0120-the-shell-asks-the-user-about-a-programs-flow.md), which starts once `netpolicy.wit` 1.3 is in `fast-test`.
- **`socket.wit` 2.8:**
  - `ask`, `asked`, `questions`, `decide`;
  - `netstack/src/questions.rs`: at most one question a grant and 8 waiting in all, a grant's own questions only, the answered ones kept for 16 more;
  - `policy.rs`: `add_rule` (never address 0 or port 0, 16 rules at most) and `spend`, which ends a rule given once at the grant's first connection through it.
- **`netpolicy.wit` 1.3:**
  - `questions` and `answer`, refused for a launcher's client;
  - the broker takes the stack's questions with its 1 s pass, so no other wakeup;
  - it names each one's program and PID;
  - it refuses a question at 60 s, and drops one whose grant ended;
  - an answer looks the host up as a line's name is, and `always` adds `program host tcp port` through `add`;
  - the program's reason reaches the log as printable ASCII only.
- **`netcheck ask:HOST:PORT:SECONDS`:** asks, waits that long for the answer, and connects if allowed.
- **Host tests:**
  - `tests/net_questions_host.rs`, 6 tests;
  - the IDL fuzzer with the new types;
  - `netpolicy_host`.
- **QEMU** (the `net` suite's `browse_check`): a question whose program ended leaves the list; one no one answers is refused at 60 s.
  - **Run locally,** x86, QEMU TCG, ef234cc8: the check alone passed, both ends with the broker's lines.
  - **On the Effector machines, 32ea39e1** (`main` f7f1d0fe merged), every group PASS:
    - host tests (`un-rizen`);
    - "x86: network, TLS and Effector" (`acc-2`);
    - "aarch64: network, TLS and Effector" (PCU);
    - "aarch64: network above 4 GiB" (`acc-1`);
    - "aarch64: GICv2 with GICv2m" (`un-rizen`);
    - "x86: services, storage, audio" (`acc-1`).

    The question's line was in the `net` suite on x86 and aarch64.
- **Left:**
  - the user's answer through the shell (APP), and with it the checks of `once`, `session` and `always`;
  - the profile's MC-11.6 row, once the whole path is tested.

## Progress (2026-10-31): in `main`

The stack's and the broker's part is in `main` at ff58632f (landing round 66; the request's commit is the branch's ff58632f). Every group of the gate passed on its first run, the `net` suite's question checks on x86 and aarch64 among them.

Still left:
- the user's answer through the shell (APP's 000-APP-0120), which starts now that `netpolicy.wit` 1.3 is in `main`;
- with it, the checks of `once`, `session` and `always`;
- the profile's MC-11.6 row, once the whole path is tested.

## Related

[351-NET-0003](../issues-done/351-NET-0003-names-in-the-network-policy.done), [108](../issues-done/108-editable-network-policy.done), [255](255-text-web-browser.md), [000-NET-0048](000-NET-0048-get-a-wget-and-curl-analog.md).
