# 000-NET-0050 — DHCP sends the machine's name (option 12)

**Type:** network (`netstack`'s DHCP client) · **Owner:** `NET` · **Priority:** P2 · **Status:** open · **Blocked by:** [000-KRN-0180](000-KRN-0180-sysmon-answers-the-machines-name.md) (the name's file and its read-only scope) · **Main task:** — · **Constitution:** MC-11.6, MC-12.1

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

## Related

[000-KRN-0180](000-KRN-0180-sysmon-answers-the-machines-name.md), 750-EFF-0018.
