# 550-NET-0041 — `wlan` joins a remembered network by itself, at start and after the network is lost

**Type:** network (service) · **Owner:** `NET` track (worked by the storage session at the maintainer's request) · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Main task:** [550](550-network-on-real-hardware.md) · **Roadmap:** track D · **Constitution:** MC-11.9, MC-12.3

From the request "`wlan` joins a remembered network by itself, at start and after the network is lost (550)" in requests-NET.md (2026-10-22). The kernel track recorded it at the maintainer's word of that day ("a NET task"), taking up the link track's request (`internal-dev-messages`, `NET/2026-10-22T1038Z-LNK-…`). This task takes it from that file.

## Problem

- **Who joins Wi-Fi today.** On the MacBook Pro, `tests/hardware/wifijoin.msh` runs as the root's `autorun.msh` and joins at start (550-ASR-0007, 000-APP-0079).
- **Why that breaks after an update.** An update through the link (351-KRN-0141) replaces the boot services in a slot, and nothing at the root. No program may write the root.
- **What happened.** After each of the first two updates, the boot's one scan missed the network and the old script gave up. The link track joined by hand over the link.
- **What exists.**
  - `wlan` already holds `keystore`'s client (slot 3, 550-KRN-0097).
  - It joins a remembered network with the kept key when `wifi join` names it with no passphrase (550-NET-0024).
  - Nothing joins without a command.

## Plan

- **Where.** In `wlan`, a boot service updated through the cable, so each release brings the join with it.
- **When it tries.** With the radio up, the station off or failed, and `keystore` remembering at least one network:
  - at start;
  - after the network is lost (beacons gone, a deauthentication: `Reason::Lost`).
- **What it does.**
  - It scans, and joins the best remembered network it hears, with the kept key, as `wifi join` does with no passphrase.
  - "Best" is the strongest heard; the most recently joined first needs a field in `keystore`'s record, which is left for later.
  - It retries every 10 s for two minutes, then every minute, until one is joined.
- **The log.** Each attempt is one line: `[WLAN] AUTO-JOIN <name>: <outcome>`, or `[WLAN] AUTO-JOIN: NONE OF <n> REMEMBERED HEARD`.
- **The user's word holds.**
  - `wifi leave` stops it until the next `wifi join` or the next boot.
  - `forget` takes the network out of it, since the remembered list is read at each attempt.
  - A word to turn it off for good (`keystore`'s record, a file, or `wifi`'s) is left for later; `wifi`'s part would be the tools track's.
- **A stand-in for a lost network.** `wlansim` (`tests/wlansim`) gets a way to fall silent for a while: no beacons and no answers, then a deauthentication. The station then sees the network lost.
- **Docs.** `docs/network/` and the `wifi` guide say what joins by itself and how to stop it.

## Acceptance criteria

- [ ] The `net` suite over `wlansim`, on x86 and aarch64:
  - after a restart of `wlan`, a remembered `MIND-TEST` is joined with no command, the attempt in the log;
  - after `wifi leave`, no join follows;
  - a lost network is joined again, each attempt in the log.
- [ ] Host tests for the retry schedule and the choice of network.

## Related

- [550-NET-0018](550-NET-0018-the-station-service.md): the station service.
- [550-NET-0024](550-NET-0024-remembered-wi-fi-networks.md): remembered networks kept by `keystore`.
- 550-ASR-0007 and 000-APP-0079: the start script on the MacBook Pro, which ASR keeps for its pings and end marker.
