# 801-NET-0084 — The station joins again, by itself, the network a client joined this boot, remembered or not; a key joins a WPA2 access point alone; a word when nothing can be joined

**Type:** `wlan`, `dot11` · **Owner:** `NET` · **Priority:** P1 · **Status:** open (2026-11-14; landed in round 138, the Ryzen's run left) · **Blocked by:** — · **Main task:** [801](801-the-bench-stays-manageable.md) (S5) · **Roadmap:** stage V · **Constitution:** MC-6.3, MC-11.6, MC-11.9

## Problem

DRV's report (`NET/2026-11-14T2047Z-DRV-801-net-0083-the-ryzens-second-loss-was-no-remembered-network.md`). The Ryzen (`fast-test` `8c067849`) was off the network from 17:28 to 20:41 UTC.

- **How it had joined.** `tests/hardware/wifijoin.msh` joined CoolNet2 with the passphrase and without `--remember`, so `keystore` kept nothing.
- **What happened.** `rtw89_wifi` restarted at 1628 s. `wlan` saw the restart (801-NET-0083), started the station anew and gave the radio its country. Then it logged nothing for three hours.
- **Why.** The new station had dropped the PMK it held for the join. The automatic join tries only the networks `keystore` keeps, and it found none, so it went quiet. A fresh `wlan` did the same.
- **The same loss without a restart.** The station's own rejoin after a loss is a few tries. When they fail, the PMK is still there, but the automatic join never uses it.

The maintainer's word for 801 is that a connection that worked must keep working. A network the user joined this boot is one that worked.

A second gap, in `dot11`. A join takes the strongest access point of the name, whatever its security. A key given for a WPA2 network could therefore join an open access point of the same name, and the automatic join of a remembered network could too.

## Plan

- **`wlan` holds the network a client joined that `keystore` does not keep:**
  - its name, and its PMK (none for an open network);
  - from the moment the station joins it, across a radio's restart and a search, until `wifi leave` or `forget` of that name;
  - replaced when a client joins another network and it joins. The PMK is wiped when it is dropped.

  The station already held that PMK for the join's own rejoins. The passphrase is not kept, and nothing goes to `keystore` (MC-11.9).
- **The automatic join tries it like a remembered network:** the one last joined first, else the strongest. Its key comes first, else the one `keystore` keeps. The try says so in a line of its own: `AUTO-JOIN <name>: THE NETWORK A CLIENT JOINED THIS BOOT, NOT REMEMBERED (TRY n)`.
- **The access point fits the key** (`dot11::station::join`, `List::best_of`):
  - with a key, held or kept, only a WPA2 access point of the name is joined;
  - with none, only an open one.

  A client's passphrase, or a network `keystore` keeps, means WPA2. A WPA2 network with no key is refused as before (`invalid`).
- **A word when there is nothing to join** (DRV's ask). With nothing remembered and nothing held, the automatic join says once per loss: `AUTO-JOIN: NO REMEMBERED NETWORK; A JOIN NEEDS THE USER`.

## Acceptance criteria

- [x] `dot11`'s host tests:
  - with an open access point of the same name stronger than the WPA2 one, a key (held or kept) joins the WPA2 one;
  - no key joins the open one;
  - a key and only an open access point join nothing and send nothing.
- [x] The QEMU `net` suite over `wlansim`, x86 and aarch64:
  - MIND-TEST is forgotten, then joined without `--remember`;
  - its radio is restarted, and the station joins it again by itself with the PMK it held. `keystore` is not asked for its keys;
  - after `wifi leave`, a fresh `wlan` says once that a join needs the user, and joins nothing.
- [x] `docs/network/station.md` says what is held, for how long, and which access point a key joins.
- [ ] On the Ryzen, the machine joined by `wifijoin.msh` without `--remember` joins again by itself after `rtw89_wifi` restarts (DRV's run).

## Progress (2026-11-15)

Landed in round 138 (`main` `3608201700`, gate green with no group failing):
- **`wlan`:** the held network (`Held`, wiped when dropped), the client's join that holds it once it joins, and the automatic join's try of it.
- **`dot11`:** `List::best_of`, and a join whose access point fits the key.
- **The checks:** `dot11`'s host test, which fails on the old station. The `net` suite's `wifi_join_check` covers the held network after a radio restart and the line said once.

Before the round, on `3608201700`:
- x86: network, TLS and Effector on acc-2, 1653 s;
- aarch64: network, TLS and Effector on PCU, 1582 s;
- host tests on x32, 2184 s.

QEMU against `wlansim` says nothing yet of a real radio (MC-12.1). The Ryzen's run, DRV's, is the last criterion. DRV was asked in `DRV/2026-11-14T2250Z-NET-801-net-0084-…`.

## Related

[801-NET-0083](801-NET-0083-the-station-never-waits-silently-after-a-radio-restart.md) (the restart found), [550-NET-0041](../issues-done/550-NET-0041-wlan-joins-a-remembered-network-by-itself.done) (the automatic join), [550-NET-0024](../issues-done/550-NET-0024-remembered-wi-fi-networks.done) (remembered networks), [801-ASR-0015](801-ASR-0015-the-bench-join-remembers-its-network.md) (the bench join remembers its network), [213-DRV-0032](213-DRV-0032-rtl8852be-wifi.md) (the receive stall itself).
