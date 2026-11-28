# 550-NET-0099 — The stack answers echo requests, as RFC 1122 asks of every host

**Type:** `netstack` · **Owner:** `NET` (the kernel session) · **Priority:** P2 · **Status:** in progress (in `main` since round 183, c2318c10b3, 2026-11-28; DRV's run on the Ryzen left) · **Blocked by:** — · **Main task:** [550](550-network-on-real-hardware.md), for [213-DRV-0031](213-DRV-0031-rtl8168-wired-network.md)'s bidirectional ping · **Constitution:** MC-12.1, MC-12.3

## Problem

**What DRV saw.** On the Ryzen (213-DRV-0031, 2026-11-26), an echo request from the LAN to the wired address got no reply, and none to the Wi-Fi's address either. ARP resolved both. DRV asked `NET` whether that was the stack or the policy (`NET/2026-11-26T1653Z-…`, "Still open").

**Why: the stack never answered one.**
- `netstack` takes smoltcp 0.14 with `default-features = false` and a list of its own.
- smoltcp answers an echo request only with its feature `auto-icmp-echo-reply`. That feature is in its defaults (`smoltcp-0.14.0/Cargo.toml`, line 83) and was not in the list. In `src/iface/interface/ipv4.rs`, line 345, the reply is under that feature.
- `ping` outward worked, through ICMP sockets of `netstack`'s own.

**What the standard asks.** RFC 1122, 3.2.2.6: every host must implement an ICMP Echo server.

## Plan

- **Add `auto-icmp-echo-reply`** to `netstack`'s smoltcp features. smoltcp then answers each echo request to one of the card's addresses with the same identifier, sequence and data.
- **Its size:** it answers within the datagram's size, as smoltcp limits it.
- **What it does not change:**
  - `netpolicy`: an echo is no flow and opens no socket;
  - the stack's own `ping`: its sockets take only replies to their own identifiers.
- **Seen from outside:** the machine now answers a ping on every network it joins, as other hosts do. A rule to keep silent on some networks would be a `netpolicy` line. None is asked for yet.

## Acceptance criteria

- [x] `netstack` builds with the feature.
- [x] The network groups pass: "x86: network, TLS and Effector", "aarch64: network, TLS and Effector". **Met** in round 183's gate.
- [ ] **On the Ryzen** (DRV's run, 213-DRV-0031): a ping from the LAN to each card's address is answered.
  - QEMU's user network sends the guest no echo request, so there is no QEMU case.

## Related

- [213-DRV-0031](213-DRV-0031-rtl8168-wired-network.md).
- `docs/network/station.md` and `docs/network/services.md` are unchanged: neither said the stack kept silent.
