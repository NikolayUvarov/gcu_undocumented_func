# 000-NET-0054 — QEMU's static setup on QEMU's cards alone, never a real card on a real network

**Type:** network · **Owner:** kernel session (`NET`) · **Priority:** P1 · **Status:** in progress (made; QEMU here, then the gate) · **Blocked by:** — · **Roadmap:** the network track · **Constitution:** MC-12.1

## Problem

DRV's report from the Ryzen 9 5900HX (2026-11-03, `ALL/2026-11-03T1817Z-DRV-…`), the RTL8168 on a real LAN:
- the cable went in at 666 s: `CARD 0 LINK UP, WAITING FOR DHCP`;
- 3 s later: `STATIC 10.0.2.15/24 GATEWAY 10.0.2.2 DNS 10.0.2.3 ON CARD 0`, QEMU user networking's addresses;
- at 680 s DHCP replaced it with `192.168.2.241/24 GATEWAY 192.168.2.1`.

**Why.** `netstack` takes that static setup on card 0 whenever 3 s pass without a lease (`FALLBACK_MS`). It is meant for QEMU, where only the `netbench` suite's tap backend has no DHCP server.

**The harm.** On a real network, a machine could for those seconds send from an address that is not its own, to a gateway that is not there, and might even collide with a real 10.0.2.15.

## Plan

1. **Only QEMU's cards.** The static setup goes only to a card whose MAC has QEMU's own prefix, `52:54:00`, its default for a card. Any other card waits for its lease.
2. **A check in the `net` suite,** `static_setup_check`, on a hub port with nothing else on it, where no DHCP answers:
   - with QEMU's default MAC the stack takes 10.0.2.15;
   - with `02:00:00:4d:43:01` it takes no address in three times that long, and at least 8 s.

## Acceptance criteria

- **The `net` suite passes on x86 and aarch64** with that check.
- **The `netbench` suite with `--tap` still gets its static 10.0.2.15** (QEMU's MAC).
- **On a real machine,** a card waits for its lease with no 10.0.2.15 line. That is evidence for that machine only (MC-12.1).

## Landed (2026-11-04)

Round 78's gate (candidate 6fc5b253, `main` e28cfe4d) passed the `net` suite with `static_setup_check` on x86 (x32-2) and aarch64 (pcu).

**Left:**
- the `netbench` suite with `--tap`, which needs a tap interface and is not in the gate;
- the Ryzen: a card that waits for its lease with no `STATIC 10.0.2.15` line.

## Related

000-NET-0044 (a card without a lease), the `netbench` suite (issue 106).
