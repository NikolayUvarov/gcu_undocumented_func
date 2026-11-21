# 801-NET-0089 — A card back within seconds on its network keeps its lease and its connections

**Type:** `netstack` · **Owner:** `NET` (the kernel session) · **Priority:** P1 · **Status:** in progress (made, 2026-11-21) · **Blocked by:** — · **Main task:** [801](801-the-bench-stays-manageable.md) (S7, S8) · **Roadmap:** stage V · **Constitution:** MC-9.3, MC-12.1, MC-12.3

## Problem

DRV's runs on the Ryzen on 2026-11-21 (`internal-dev-messages`, `NET/2026-11-21T1213Z-DRV-what-the-ryzens-wi-fi-runs-showed-of-netstack-and-wlan.md`, 213-DRV-0032):
- each time `rtw89_wifi` stopped and `init` started it again, the station had no link for 2 to 5 s;
- `netstack` then said `CARD 2 LINK DOWN: ADDRESS DROPPED, 1 CONNECTIONS ABORTED`, and Effector's agent opened a new TLS session;
- a download of 30 MB never fitted between two restarts, and the agent started again from byte 0. EFF is adding a resumed download (801-EFF-0037).

The stack drops a card's address and aborts its TCP connections the moment its link goes down (550-NET-0023). That is right for a network that is gone, and wrong for one that comes back in seconds: the same access point, the same lease, and peers that would have waited.

## Plan

1. **A link down with a lease holds it for 30 s.**
   - The address, the routes and the TCP connections stay.
   - New flows take another card meanwhile, as the route needs a link that is up.
   - Logged: `CARD n LINK DOWN: ADDRESS AND k CONNECTIONS HELD FOR 30 S`.
2. **The link back within the hold: DHCP from the start, the old address kept until it answers.**
   - The same address and gateway keep the connections: `CARD n BACK ON ITS NETWORK AFTER t MS: ADDRESS AND k CONNECTIONS KEPT`.
   - Another address or gateway aborts them, as another network does, and the new lease is applied.
3. **No lease by the end of the hold,** link down or not: the address goes and the connections are aborted, as before. Logged: `LINK STILL DOWN AFTER 30 S` or `HAS NO LEASE OF ITS ADDRESS AFTER 30 S`.
4. **Unchanged:**
   - a card with no lease, such as QEMU's static setup, is dropped at once;
   - a card taken out of use (550-NET-0062) is dropped at once.

## Acceptance criteria

- [x] **QEMU, x86 and aarch64, the `net` suite's two cards:**
  - a link down 3 s keeps card 0's lease, and the connection `netcheck hold` keeps goes on: every byte it sent reaches the host, those sent while the link was down included;
  - down past 30 s, the address goes and the connection is aborted;
  - the default route moves to the other card while the link is down and comes back with it;
  - DHCP's name check (000-NET-0050) passes with the held lease asked for again.
- [x] The `net` groups pass on both architectures in the gate, and the Wi-Fi checks with `wlansim`.
- [x] docs/network (EN) say what the stack does when a link goes down.

**Not covered:**
- a radio driver's restart on the Ryzen (213-DRV-0032), which DRV can run once this lands;
- a peer that gives up on its side during the hold (its own timers);
- `wifi leave` holds the lease too: the stack sees only the link.


## Landed (2026-11-21)

In main since round 166 (6b84acb734, `LANDING/2026-11-21T1745Z-KRN-landed-e09e26df.md` on `internal-dev-messages`). The gate passed every group on both architectures, among them x86's and aarch64's network groups with `wlansim` and the host tests. So a stick written from main carries it, and no `fast-test` build is needed.

**Left:** a card's link going down and coming back on a real machine (the Ryzen's Wi-Fi restart, DRV).

## Related

[550-NET-0023](../issues-done/550-NET-0023-a-cards-link-down-and-up.done) (the link down and up), [550-NET-0062](../issues-done/550-NET-0062-the-card-chosen-in-software.done) (a card out of use), [801-NET-0088](../issues-done/801-NET-0088-a-long-download-over-the-dexps-wi-fi-is-cut-by-a-stall.done), 801-EFF-0037 (EFF's resumed download), 213-DRV-0032.
