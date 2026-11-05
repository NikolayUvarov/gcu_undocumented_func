# 000-NET-0050 — DHCP sends the machine's name (option 12)

**Type:** network (`netstack`'s DHCP client) · **Owner:** `NET` · **Priority:** P2 · **Status:** in progress · **Blocked by:** [000-KRN-0180](000-KRN-0180-sysmon-answers-the-machines-name.md) (the name's file and its read-only scope) · **Main task:** — · **Constitution:** MC-11.6, MC-12.1

**Asked by:** EFF with LNK, `ALL/2026-10-28T0407Z-EFF-the-machines-name.md` on `internal-dev-messages`, item 4.

## Problem

- **What the router sees.** The DHCP client sends no name, so a router lists the machine by its MAC address alone.
- **What the maintainer wants.** On the DEXP: `effector485 submit shell line="hostname dexp"`, and then the router's lease shows `dexp`.

## Plan

- **`netstack` sends the name as option 12** (Host Name, RFC 2132) in DISCOVER and REQUEST. It reads the name through its read-only scope of the name's directory (000-KRN-0180), as one RFC 1123 label.
- **The name is read again for each lease,** so a changed name reaches the router at the next renewal.
- **Without a valid name,** `mindcore` is sent.
- **The log says it:** `[NETSTACK] DHCP AS <name>`.

## Acceptance criteria

- **QEMU, x86 and aarch64:**
  - with the name `dexp`, QEMU's user network sees option 12 in the DHCP exchange (a capture of the slirp traffic, or the suite's own DHCP server);
  - the log line names it.
- **On the DEXP:** the router's lease shows the name, recorded with the router's model (MC-12.1).

## Progress

2026-11-05, on `claude/000-NET-0050-dhcp-name` (on 000-KRN-0180's branch):
- **`netstack`** (`netstack/src/name.rs`):
  - it reads `name` through `init`'s read-only scope of `data/machine/`, with `mind::hostname`'s rule. The scope is in `SLOT_FILE`, as slot 3 is its second card;
  - every card's DHCP client sends it as option 12 (`set_outgoing_options`), so each DISCOVER, REQUEST and renewal carries it;
  - **when:** at its start, when a link comes back up (before its new DISCOVER), and every 60 s;
  - a read waits at most 500 ms, and one that fails keeps the name as it was: the stack does not wait on files;
  - each change logs `[NETSTACK] NAME FOR DHCP: <name> (<source>)`. The plan's `[NETSTACK] DHCP AS` began as the lease's line does, which the suites wait for: the first x86 run's wait for the lease ended on it;
  - **without a name,** `mindcore` is sent.
- **Memory:** smoltcp keeps the option by reference for the socket's life, so each new name's few bytes stay allocated.
- **`init`:** `scopes_of("netstack")`, made only below a `data/` that is there, as `sysmon`'s is (000-KRN-0180).
- **The check** is `dhcp_name_check` in the `net` suite:
  - QEMU's `filter-dump` records the card's traffic, and `dhcp_sent` reads the client's DHCP messages from it;
  - with `dexp` on the disk, every DISCOVER and REQUEST at boot carries it;
  - `lab-2` written, then the link down and up: the new lease's messages carry `lab-2`;
  - `stat caps` shows one files client in slot 7, the scope's.
- **Docs:** `docs/profile/bootstrap.md` (`netstack`), `docs/profile/evidence.md`, `README.md`.

**Left:** QEMU on x86 and aarch64; the DEXP's router (the second criterion), on the maintainer's word.

## Related

[000-KRN-0180](000-KRN-0180-sysmon-answers-the-machines-name.md), 750-EFF-0018.
