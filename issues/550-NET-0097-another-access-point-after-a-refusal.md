# 550-NET-0097 — After a try fails on one access point, the automatic join takes another of the same network

**Type:** `wlan`, `dot11` · **Owner:** `NET` (the kernel session) · **Priority:** P1 · **Status:** in progress (in `main` since round 183, c2318c10b3, 2026-11-28; DRV's run on the Ryzen left) · **Blocked by:** — · **Main task:** [550](550-network-on-real-hardware.md), for [801](801-the-bench-stays-manageable.md) · **Constitution:** MC-12.1, MC-12.3

## Problem

DRV's report `NET/2026-11-27T0547Z-DRV-the-access-point-moved-to-channel-40-under-00-…` on `internal-dev-messages`. It holds for the Ryzen, release 202611270533 (`wlan` and `dot11` as in `main` 9d6feac696), and the maintainer's access point CoolNet2.

- **What happened.** The access point's 5 GHz side moved overnight from channel 100 to channel 40. Under the kept country `00`, `rtw89` refuses channel 40: its calibration does not fit the channel's world maximum (213-DRV-0087). The driver ends, and `init` starts it again.
- **The loop.**
  1. Each try of the automatic join ended with the radio's restart, or with `NO ANSWER`.
  2. The next try joined the same access point on channel 40 again.
  3. After 3 restarts in 60 s, `init` quarantined the driver for 240 s.
  4. For nine minutes the station never tried the same network's 2.4 GHz access point on channel 8. The boot before had joined it.
- **The cost.** The machine stayed reachable only because its wire was in. Without the wire it would have been off the network until someone typed at it. That is 801's case.
- **Why.** The automatic join chose the network by name. `dot11`'s station then took the network's first access point by 550-NET-0090's order (5 GHz above −70 dBm first), every time. Nothing kept that the access point had just failed. The station is made anew at each restart of the radio, so it could not keep it either.

## Plan

1. **`dot11`:** `List::best_avoiding` and `Station::join_avoiding`. They take another access point of the network before any in a list given. One in the list is taken only when the network has no other, or all of its access points are in the list. `best_of` and `join` are the same with an empty list.
2. **`wlan`:**
   - It keeps the access point (BSSID, channel) its try is joining.
   - It sets that access point aside for 10 minutes (`auto::SetAside`, at most 4) when the try:
     - ends with the radio's restart;
     - or ends with `NO ANSWER` or `REFUSED`.
   - It logs each one it sets aside, with the reason.
   - The next try joins avoiding those set aside.
   - A join lets every one go.
   - The list lives in `wlan`, so it outlives the radio's restart.
3. **Unchanged:**
   - a client's `wifi join`;
   - the station's own rejoin of a network just lost;
   - 550-NET-0090's order whenever nothing is set aside.

## Acceptance criteria

- [x] Host tests:
  - `dot11/tests/station.rs` `an_access_point_set_aside_goes_after_the_networks_others`: channel 40 set aside gives channel 8; both set aside, or a network with one access point, give the usual order.
  - `tests/wlan_auto_host.rs` `an_access_point_set_aside_waits_its_time_and_a_join_lets_all_go`: the time, a second setting aside, the oldest giving way, the join.
- [x] `wlan` builds with no warning.
- [x] The groups that run the `net` suite's Wi-Fi checks pass: "x86: network, TLS and Effector" and "aarch64: network, TLS and Effector". **Met** in round 183's gate (x86 on acc-2, 2175 s; aarch64 on acc-1, 1598 s), QEMU only (MC-12.1).
- [ ] **On the Ryzen** (DRV's run, MC-12.1): with the access point on a channel the radio refuses under `00`, the station sets it aside after one try and joins the 2.4 GHz access point on the next.

## Related

- [213-DRV-0087](213-DRV-0087-rtl8852be-on-5-ghz.md): the radio's side, why channel 40 is refused under `00`.
- [550-NET-0090](550-NET-0090-the-station-on-5-ghz.md): the order of a network's access points.
- [550-NET-0041](../issues-done/550-NET-0041-wlan-joins-a-remembered-network-by-itself.done): the automatic join.
- `docs/network/station.md` 0.7.
