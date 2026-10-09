# The network policy

**Version:** 0.1 (2026-10-09) · **Track:** `NET`, main task [108](../../issues-done/108-editable-network-policy.done) · **Roadmap:** track D ("names and editable policy") · **Constitution:** MC-11.6, MC-3.11, Appendix B.6

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

A host name is looked up when the grant is made (351-NET-0003). The grant keeps the address it got, and the log names both. The answer is plain DNS: the grant reaches whatever address the resolver gave.

## Changing it

```
netpolicy                      # the lines in force
netpolicy add <line>           # asks, then adds the line at the end
netpolicy remove <line>        # asks, then removes every equal line
```

- **The shell asks first.** It prints `ADD THE NETWORK POLICY LINE "…"? (Y/N)` and waits for a key on the keyboard or the serial line, which programs cannot type into. With no answer in a minute, or `N`, nothing changes. A script that runs `netpolicy add` gets the same question.
- **Only the shell holds a client of the broker.** `init` lends it to the shell only, and the shell never lends it to a program. `idl/netpolicy.wit` 1.1 adds `lines`, `add` and `remove` to it.
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

In the `tls` suite's raw-image check, a change is still there after a reboot.

## Not provided

- **Authenticated name answers.**
- **Policies per user, or roles:** whoever is at the keyboard decides.
- **Limits on the shell's operator client,** which reaches every destination.
- **A change that applies to grants already made:** `netrevoke` ends them.
