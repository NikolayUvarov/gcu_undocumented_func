# 550-NET-0039 — `wlan` joins a remembered network by itself, at start and after the network is lost

**Type:** network (service) · **Owner:** kernel session, owner of `NET` (the maintainer, 2026-10-22) · **Priority:** P1 · **Status:** open (2026-10-22) · **Blocked by:** — · **Main task:** [550](550-network-on-real-hardware.md) · **Roadmap:** track D · **Constitution:** MC-11.9, MC-12.3

From the link track's request (`internal-dev-messages`, `NET/2026-10-22T1038Z-LNK-wlan-joins-a-remembered-network-by-itself.md`), which the maintainer made a NET task on 2026-10-22. It was recorded in [requests-NET.md](requests-NET.md) and is numbered here.

## Problem

- **Who joins Wi-Fi today.** The MacBook Pro joins at start through `tests/hardware/wifijoin.msh`, run as `autorun.msh` from the volume's root (550-ASR-0007, 000-APP-0079).
- **Why that breaks after an update.** An update through the link (351-KRN-0141) replaces the boot services in a slot, but nothing at the root, which no program may write.
- **What happened.** After each of the first two updates, the boot's one scan missed the network, and the old script gave up. The link track joined by hand.
- **What the station does today.**
  - It rejoins a lost network once, 3 s after it is lost (`dot11::station`), then stays failed.
  - It joins only when `wifi join` asks.

## The maintainer's decision (2026-10-22)

- **Where it goes.** `wlan`, a boot service updated through the cable, joins a remembered network by itself:
  - at start;
  - again after the network is lost.
- **How.** It retries, and logs each try.
- **The user's word holds.** `wifi leave` and `forget` stay in force.

## Plan

- **`wlan/src/auto.rs`** holds the rules, with no I/O:
  - **The schedule.** A try every 10 s for the first two minutes of a search, then one a minute. A search begins at start, and again when a joined network is lost.
  - **The choice.** The network last joined, when it is heard; else the strongest remembered network heard.
  - **What stops it.** `wifi leave` turns it off until the station next joins a network. Joining turns it on again and ends the search.
- **`wlan`.**
  - A try runs only when the station is neither joined nor busy: not scanning, not joining, and not in its own rejoin of a network just lost. Each try is a scan, then a join of the chosen network with its kept key (`Secret::Kept`, 550-NET-0024).
  - Each try ends in one line of the log, e.g. `[WLAN] AUTO-JOIN MIND-TEST: JOINED (TRY 3)` or `[WLAN] AUTO-JOIN: NO REMEMBERED NETWORK HEARD, 1 KEPT (TRY 1), AGAIN IN 10 S`.
  - **When nothing is tried.** With no network remembered, or no answer from `keystore`, there is no try and no line. Without a random source, the station says once that it joins nothing by itself: a remembered network is WPA2's, and its handshake needs one (issue 103).
  - **A client's join** stops an automatic scan under way, so the station takes the client's join.
  - **`forget`** takes the network out of the choice at the next try.
- **`tests/wlansim`.** The stand-in access point goes away when the station asks (ARP) for 10.0.4.98: a deauthentication, then 25 s with no beacon and no answer, while the radio answers its calls.

## Acceptance criteria

- [ ] Host test `tests/wlan_auto_host.rs`: the schedule, leave and join, and the choice (the previous network, the strongest, an unknown signal, a hidden network).
- [ ] The `net` suite over `wlansim`, x86 and aarch64:
  - after `keystore` and `wlan` restart, the remembered `MIND-TEST` is joined with no command, and the gateway answers;
  - with the access point away, the station's own rejoin fails, the failed tries are logged with their numbers, and the network is joined again by itself;
  - after `wifi leave`, nothing is joined by itself for 25 s, and `wifi status` says `OFF`.
- [ ] The gate passes.

## Related

[550-NET-0018](550-NET-0018-the-station-service.md), [550-NET-0024](550-NET-0024-remembered-wi-fi-networks.md), 550-ASR-0007, 000-APP-0079, [351-KRN-0141](351-KRN-0141-an-update-through-the-link.md).
