# 550-NET-0090 — The station on 5 GHz: the network's 5 GHz access point first, and a rejoin on a radar channel only after its beacon

**Type:** `dot11` (the station) · **Owner:** `NET` (the kernel session) · **Priority:** P1 · **Status:** in progress (made, 2026-11-21) · **Blocked by:** — · **Main task:** [550](550-network-on-real-hardware.md) · **Constitution:** MC-6.1, MC-12.1, MC-12.3

## Problem

DRV starts [213-DRV-0087](213-DRV-0087-rtl8852be-on-5-ghz.md): the Ryzen's RTL8852BE on 5 GHz. The Ryzen's network, CoolNet2, is on channel 8 and on channel 100. The 5 GHz one is neither crowded nor shared with Bluetooth. NET's answer of 2026-11-21 (`DRV/2026-11-21T1304Z-NET-re-wlan-on-5-ghz.md` on `internal-dev-messages`) found two gaps in the station.

1. **The strongest access point wins, whatever its band.** A 5 GHz signal is usually the weaker, so the automatic join and `wifi join` take channel 8.
2. **A rejoin sends at once.** The station rejoins a network silent for 10 s 3 s later, and sends authentication on its channel without hearing a beacon there again. On a radar channel (DFS, 52 to 144; channel 100 among them) the access point may have left after radar. The station may send there only to an access point it has heard.

## Plan

1. **A 5 GHz access point of the network first,** when its signal is at least `scan::FIVE_GHZ_FLOOR` (−70 dBm); otherwise the strongest, as before. This holds for `List::best` and `best_of`, so for `wifi join` and the automatic join alike. The floor is to be set from DRV's runs.
2. **A rejoin on a listen-only channel** (DFS or NO-IR under the country):
   - it tunes there and waits up to `BEACON_WAIT_MS` (1 s) for the network's beacon, sending nothing;
   - with the beacon, the join goes on;
   - without it, the station fails with `Lost`, having sent nothing, and `wlan`'s search, which only listens there, takes over;
   - a leave meanwhile sends no deauthentication.

   On a channel the station may send on, the rejoin is as before.

**Not done:** following a channel switch announcement (the network is found again by the search); HT and VHT.

## Acceptance criteria

- [x] Host (`dot11/tests/station.rs`):
  - of one network on channels 8 and 100, the 5 GHz access point is taken above the floor, the strongest below it, and the strongest of several 5 GHz ones;
  - on a listen-only channel, a rejoin sends nothing before the beacon, joins after it, and fails with nothing sent without it;
  - on a sendable channel, a rejoin is as before.
- [x] The `net` suite's Wi-Fi checks with `wlansim`, on both architectures, in the gate.
- [ ] **On the Ryzen, with 213-DRV-0087:** the station joins CoolNet2 on channel 100 when both are heard (MC-12.1: that machine and that access point).
- [x] docs/network/station.md says which access point is joined, and the rejoin's rule.


## Landed (2026-11-21)

In main since round 166 (6b84acb734, `LANDING/2026-11-21T1745Z-KRN-landed-e09e26df.md` on `internal-dev-messages`). The gate passed every group on both architectures, among them x86's and aarch64's network groups with `wlansim` and the host tests. So a stick written from main carries it, and no `fast-test` build is needed.

**Left:** the Ryzen joining CoolNet2 on channel 100 (DRV, with 213-DRV-0087).

## Related

[213-DRV-0087](213-DRV-0087-rtl8852be-on-5-ghz.md), [550-NET-0035](../issues-done/550-NET-0035-the-stations-country.done) (the country's listen-only channels), [550-NET-0041](../issues-done/550-NET-0041-wlan-joins-a-remembered-network-by-itself.done) (the automatic join).
