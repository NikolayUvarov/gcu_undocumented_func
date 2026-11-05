# The network policy

**Version:** 0.7 (2026-11-04: the time servers of `timed`, 111-NET-0057; 0.6: 2026-10-31, a question at run time, the stack's and the broker's part, 000-NET-0049; 0.5: 2026-10-28, the browsing grant; 0.4: 2026-10-21, a launcher's client, 800-NET-0038; 0.3: 2026-10-12; 0.1: 2026-10-09) · **Track:** `NET`, main task [108](../../issues-done/108-editable-network-policy.done), the bounds of a flow [750-NET-0014](../../issues-done/750-NET-0014-the-bounds-of-a-flow.done), the default card [550-NET-0025](../../issues-done/550-NET-0025-the-default-card.done) · **Roadmap:** track D ("names and editable policy") · **Constitution:** MC-11.6, MC-3.11, Appendix B.6

A program reaches the network only through a flow grant. It asks for one with `REQUEST_NETWORK`, its launcher asks the policy broker `netpolicy`, and the broker makes it from the policy. This page describes the policy, its lines, and how it changes while the system runs.

## Where it is

- **`netpolicy.txt`** at the root of the boot disk is the shipped policy. No program can write the boot disk outside `data/`.
- **`system/netpolicy/netpolicy.txt`** is the policy as changed. The broker reads it in place of the shipped one once a change has been made. It lives in the broker's private directory: `vfs_server` lets only the broker's badge open it, and the shell and programs cannot even read it (351-NET-0005).
- The broker reads the policy again for every grant. A grant made before a change keeps its rules until it ends; the next grant follows the change.

## The lines

One line per rule, `#` starts a comment:

| Line | Meaning |
|---|---|
| `program address tcp\|udp\|icmp [port [seconds [bytes]]]` | `program` may reach `address` (an IPv4 address or a host name) over the protocol, on `port` (0 or none: any), for a term of `seconds` (default 3600) and a volume of `bytes` (default 16 MiB); a grant takes the shortest term and the smallest volume its lines give |
| `program dns` | `program` may ask the stack's DNS server (UDP port 53) |
| `resolver A.B.C.D[:PORT]` | host names in this file are looked up at this server (default: the stack's DNS server) |
| `default card N` | card `N` (0 or 1: the wired cards, 2: the Wi-Fi station) is the default while its link is up and it has an address and a gateway; the last such line counts (550-NET-0025) |
| `browse program public tcp PORT[,PORT…] [seconds [bytes]]` | `program` may reach **any public address** over TCP on these ports (at most 4, none 0): a browsing grant (000-NET-0049, below) |
| `listen program tcp PORT from A.B.C.D/BITS[,A.B.C.D/BITS…] [seconds [bytes]]` | `program` may **listen** on TCP `PORT` and take connections from peers in these networks (at most 4 a port, host bits 0; `0.0.0.0/0`: anywhere): a listening grant (110-NET-0052, below) |

### The browsing grant

- **What it reaches.** Any public IPv4 address on its ports, and nothing else (`idl/socket.wit` 2.7, a rule of address 0, TCP only).
- **Not public:**
  - this network (0/8), the private ranges (10/8, 172.16/12, 192.168/16) and shared space (100.64/10);
  - loopback (127/8) and link-local (169.254/16);
  - the IETF's assignments (192.0.0/24), the documentation ranges (192.0.2/24, 198.51.100/24, 203.0.113/24) and benchmarking (198.18/15);
  - multicast (224/4), and the reserved block with the broadcast address (240/4) (`mind::network::public`).
- **A private host** is reached only through a line of its own that names it.
- **The audit.** The stack logs each new host a grant reaches through its browsing rule, once per host: `[NETSTACK] GRANT n BROWSES A NEW PUBLIC HOST A.B.C.D:PORT (k SO FAR)`. It remembers the last 256 hosts of a grant, so past them a host can be logged again.
- **The broker's line** for such a grant says so: `[NETPOLICY] GRANT n TO program: k RULES, BROWSING ANY PUBLIC ADDRESS, …`.
- **Names.** A program that looks names up needs `program dns` too. The answer is plain DNS, as for any grant.

### The listening grant

A server on MIND Core (a web server, an SSH server) listens through such a grant (110-NET-0052, `idl/socket.wit` 2.9).
- **Listening.** The program calls `tcp-listen(port, backlog)` for a port its lines name, with a backlog of 1 to 16 connections set up and not yet accepted. It listens on the default card.
- **Taking a connection.** `tcp-accept` hands over the oldest connection set up, as a connected socket with the peer's address and port. It is used and charged as a connection the program made, within its grant's term and volume.
- **What the stack refuses:**
  - a port no line names: `tcp-listen` is `denied`;
  - a port another socket listens on: `refused`;
  - a peer outside the port's networks: reset as soon as the stack sees it, logged as `[NETSTACK] PORT p REFUSED A.B.C.D:PORT (BADGE n): NOT A NETWORK ITS GRANT NAMES`;
  - a SYN to a port nothing listens on: answered with a reset.
- **Limits:**
  - a backlog of at most 16;
  - at most 32 listening sockets in the stack, against a flood of SYNs;
  - a connection set up no further than 10 s is dropped;
  - a program holds at most 8 sockets, the accepted ones among them.
- **The audit:**
  - the broker logs each port a grant may listen on: `[NETPOLICY] GRANT n TO program: LISTENS ON TCP p FROM A.B.C.D/BITS`;
  - the stack logs each listening socket (`PORT p LISTENS FOR PID …`) and each connection it accepts (`PORT p ACCEPTED A.B.C.D:PORT FOR PID …`).
- **Tested:**
  - **in QEMU** (the `net` suite's `listen_check`, x86 and aarch64): a connection through QEMU's forwarded port accepted and charged, a port no line names denied, a peer outside the networks reset;
  - **on the host** (`tests/listen_host.rs`, smoltcp's own TCP over a loopback wire that can lose a client's acknowledgements):
    - a backlog of two holds two connections and a third peer is reset;
    - a peer outside the networks is reset and never ready;
    - two connections left half open hold the backlog until they go after 10 s;
    - the bounds on the port, the backlog and the listening sockets.
  - Not on a machine (MC-12.1).

### The time servers

The network time service `timed` (111-NET-0057) asks the servers its lines name, and sets the calendar clock from them.
- **Its lines** are ordinary rules of program `timed`, one a server: `timed 192.0.2.10 udp 123 4294967295`. Give each the longest term: the grant is made once at boot and is kept for the service's restarts.
- **Without such a line** `init` does not start it: `[INIT] timed NOT STARTED: THE POLICY GIVES IT NO TIME SERVER`.
- **It reads them back** from its own grant (`grant-rules`, `idl/socket.wit` 2.10): its UDP rules with an address, at most 4, each on its port (123 when a line names none).
- **What it takes** (`docs/profile/clocks.md`): at least two servers that agree within a second, never a time before its floor, and a step of more than 24 hours only at the first set after boot.

### A question at run time

A program that reaches a host the user names only when it runs (`get URL`, a mail client with the user's server) may ask for it while it runs (000-NET-0049).

- **The program asks through its grant** (`idl/socket.wit` 2.8): `ask(host, port, why)` for one TCP port. The host is a name or a dotted address. The reason is the program's own words, at most 80 bytes. The program then reads the answer with `asked(id)`: pending, allowed with the address to reach, or refused.
- **The stack decides nothing.** It keeps the question, at most one a grant and 8 in all, and refuses one from a client that is not a live grant.
- **The broker** (`idl/netpolicy.wit` 1.3):
  - takes the questions from the stack once a second;
  - names each one's program and PID from its grants;
  - logs it: `[NETPOLICY] QUESTION n: program (PID p) ASKS FOR host:port TCP: "why"`;
  - serves the questions (`questions`) and takes the answer (`answer`), for the shell's client only. A launcher's client is refused both.
- **The answers:**
  - **no** refuses it;
  - **once** adds a rule that ends with the grant's first connection through it;
  - **session** adds a rule for the rest of the grant, which ends with the program;
  - **always** adds the rule and the line `program host tcp port` to the policy, as `netpolicy add` would.

  A name is looked up when the answer comes, as for a line.
- **A question no one answers in 60 s is refused** (`NO ANSWER IN 60 S, REFUSED`), and one whose program ended leaves the list (`ITS PROGRAM ENDED`).
- **The shell puts the question to the user** (000-APP-0120). It asks the broker for questions every 500 ms and shows the oldest of a running program on the console that program was started from, or in the shell's window beside a window manager it started.
  - `N`, Esc or Enter refuse; `O` allows once, `S` for the session, `A` always.
  - Any other key is held back while the question shows, and a key in its first 500 ms does not answer it.
  - The reason is shown with every control character as `?`.
  - A question whose program ended, or that is refused at 60 s, is taken down with a line saying so.

### Term and volume

- **A term** is 0 to 4294967295 seconds (about 136 years). It runs from the moment the broker makes the grant, on the stack's monotonic clock, and nothing renews it. A term of 0 gives a grant that has already ended.
- **A volume** is 0 to 18446744073709551615 bytes (2^64 − 1). It counts every byte the grant's sockets send and receive together, payload only, over the grant's life.
- **When either runs out,** the stack closes the grant's sockets and refuses its requests (`[NETSTACK] GRANT n ENDED, k SOCKETS CLOSED`), and the broker removes the grant (`[NETPOLICY] EXPIRED …`).
- **A long-running service** (a client of one server for as long as the machine runs) gets its own line with a long term and a large volume, for example `effector 203.0.113.7 tcp 443 4294967295 1099511627776`. Its grant then lasts as long as the line says, or until `netrevoke` ends it, or `netstack` or the system restarts. A program the shell starts loses its grant when its process ends; a boot service `init` starts keeps the same grant across its restarts, as `init` keeps its plan, so a restart renews neither its term nor its volume. With two lines for one program, the smaller volume is the budget of both (EFF's restarted session, 2026-10-22).
- **What a grant reports** of its term (`netgrants`, `idl/netpolicy.wit` `left-ms`, `idl/socket.wit` `policy-usage`) is in milliseconds as a u32, so a term longer than 49.7 days shows as 4294967 s left until it comes within that.

### The default card

- **The rule.** A flow goes out through the card whose network holds its destination. Any other flow, and the stack's DNS server, go through the default card: the preferred card while its link is up and it has an address and a gateway, else the first card that has, wired cards (0, 1) before the Wi-Fi station (2), else any card with an address.
- **The preferred card** is the policy's `default card N` line. The broker tells the stack at start, after each change and every 10 seconds, so a restarted stack learns it again. Without the line no card is preferred.
- **Logged:** `[NETPOLICY] DEFAULT CARD N FROM THE POLICY`, `[NETSTACK] CARD N PREFERRED AS THE DEFAULT`, and each move of the default, `[NETSTACK] DEFAULT ROUTE: CARD N`.
- **`idl/socket.wit` 2.5** `cards` tells each card's kind (wired or wireless), whether it is there, up and configured, and which one is preferred and which is the default; `netcheck cards` prints them.

A host name is looked up when the grant is made (351-NET-0003). The grant keeps the address it got, and the log names both. The answer is plain DNS: the grant reaches whatever address the resolver gave.

## Changing it

```
netpolicy                      # the lines in force
netpolicy add <line>           # asks, then adds the line at the end
netpolicy remove <line>        # asks, then removes every equal line
```

- **The shell asks first.** It prints `ADD THE NETWORK POLICY LINE "…"? (Y/N)` and waits for a key on the keyboard or the serial line, which programs cannot type into. With no answer in a minute, or `N`, nothing changes. A script that runs `netpolicy add` gets the same question.
- **Only a client with no badge may change the policy** or revoke a grant (`idl/netpolicy.wit` 1.1 adds `lines`, `add` and `remove`). `init` lends that client to the shell, which asks first and never lends it to a program.
- **A launcher's client may only launch** (`idl/netpolicy.wit` 1.2, `mind::network::BADGE_LAUNCH`, 800-NET-0038). It asks for grants for the programs it starts and reads the lines and the grants. `add`, `remove` and `revoke` are refused (`denied`) and logged: `[NETPOLICY] ADD REFUSED FOR PID n: A LAUNCH-ONLY CLIENT`. So is a client with any other badge. A grant it asks for is logged with `, ASKED BY A LAUNCH-ONLY CLIENT`.
- **`link` in its developer mode** is to hold such a client, as it has no user to ask (800-KRN-0139). Until `init` mints it with the badge (800-KRN-0142), `link` holds the shell's kind.
- **The broker checks every line.** It refuses a line it does not understand (`Invalid`), and a comment in an added line.
- **A change is stored whole.** It is written beside the old file, which is then replaced. A cut between the two leaves the new text in `netpolicy.new`, which the broker reads in that case.
- **Every change is logged:** `[NETPOLICY] POLICY CHANGED BY PID n: ADDED …`, or `REMOVED … (k LINES)`. So is a refused one: `CHANGE REFUSED FOR PID n: NOT A POLICY LINE: …`.

## Tested

In the `net` suite on x86 and aarch64:
- a change refused by the user is not made;
- an added line gives the next grant its rule;
- a line that is not policy is refused;
- a removed line takes the rule away again;
- the shell cannot open `system/netpolicy`;
- every change is logged.

In the `net` suite's two-card check (550-NET-0025), on x86 and aarch64:
- with no preference, card 0 is the default and datagrams to an address on neither card's network leave through it;
- `default card 1` added through `netpolicy add` makes card 1 the default, and they leave through card 1;
- with card 1's link down, card 0 is the default again, and card 1 again once its link is back;
- `default card 3` is refused, and removing the line makes card 0 the default again.

In the same suite (750-NET-0014), on x86 and aarch64:
- a line with the largest term and volume gives a grant with them;
- a grant's 10-second term ends the connection it holds, while a lasting grant holds its connection for 40 s.

A connection held past the default term (an hour) is not run.

In the `net` suite's browsing check (000-NET-0049), on x86 and aarch64, before the shell asked:
- `netcheck` asked for a port its grant does not name; the broker logged the question with the program and its PID;
- it ended without an answer, and the question left the broker's list;
- asked again and left unanswered, it was refused at 60 s.

With the shell's part (000-APP-0120, `main` 9a620b0e, round 73), the same check now answers it every way, on x86 and aarch64:
- the question is shown on the program's console, and taken down at the program's end;
- `N` refuses it;
- `O` allows one connection, and the next of the same run is `Denied`;
- `S` allows the rest of the run;
- no answer is refused at 60 s;
- `A` writes `netcheck 11.0.2.80 tcp 81` to the policy, and a new run reaches the host with no question.

In the `tls` suite's raw-image check, a change is still there after a reboot.

## Not provided

- **Authenticated name answers.**
- **Policies per user, or roles:** whoever is at the keyboard decides.
- **Limits on the shell's operator client,** which reaches every destination.
- **A change that applies to grants already made:** `netrevoke` ends them.
- **A renewable grant, a volume per period, a name looked up again on reconnecting:** a long-running service holds one grant for its life (750-NET-0014).
- **Listening on another card than the default one, or on every card:** a listening socket takes connections on the card that was the default when it was opened.
- **A refusal before the handshake:** smoltcp answers a SYN on a listening port before the stack sees the peer, so a peer outside the grant's networks may see its connection set up, then reset.
- **Grants across a restart of `netstack`:** the stack keeps them in memory only, so after a restart a grant's requests are refused until its program is started again with a new one (read from the code, not tested).
