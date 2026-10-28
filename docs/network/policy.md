# The network policy

**Version:** 0.5 (2026-10-28: the browsing grant, 000-NET-0049; 0.4: 2026-10-21, a launcher's client, 800-NET-0038; 0.3: 2026-10-12; 0.1: 2026-10-09) · **Track:** `NET`, main task [108](../../issues-done/108-editable-network-policy.done), the bounds of a flow [750-NET-0014](../../issues-done/750-NET-0014-the-bounds-of-a-flow.done), the default card [550-NET-0025](../../issues-done/550-NET-0025-the-default-card.done) · **Roadmap:** track D ("names and editable policy") · **Constitution:** MC-11.6, MC-3.11, Appendix B.6

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
- **Not done:** asking the user for a host at run time, the rest of 000-NET-0049.

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

In the `tls` suite's raw-image check, a change is still there after a reboot.

## Not provided

- **Authenticated name answers.**
- **Policies per user, or roles:** whoever is at the keyboard decides.
- **Limits on the shell's operator client,** which reaches every destination.
- **A change that applies to grants already made:** `netrevoke` ends them.
- **A renewable grant, a volume per period, a name looked up again on reconnecting:** a long-running service holds one grant for its life (750-NET-0014).
- **Grants across a restart of `netstack`:** the stack keeps them in memory only, so after a restart a grant's requests are refused until its program is started again with a new one (read from the code, not tested).
