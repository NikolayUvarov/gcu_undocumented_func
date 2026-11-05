# The Wi-Fi station: what joins by itself

**Version:** 0.2 (2026-11-05) · **Track:** `NET`, tasks [550-NET-0041](../../issues-done/550-NET-0041-wlan-joins-a-remembered-network-by-itself.done) and [550-NET-0058](../../issues-done/550-NET-0058-a-read-only-wifi-client.done) (who may change the station) for main task [550](../../issues/550-network-on-real-hardware.md) · **Roadmap:** track D · **Constitution:** MC-11.9, MC-12.3

`wlan` is the 802.11 station, a boot service that runs when a radio does (`bcm_wifi`, or `wlansim` in the tests). A program joins a network through its `idl/wifi.wit` client (the `wifi` tool). The station also joins a network by itself, as this page describes. It is a boot service, so every release, including one put through the link, brings this behaviour with it. A script at the volume's root does not.

## When it joins by itself

- **At start, and after a network is lost.** A lost network means its beacons are gone, or it sent a deauthentication.
  - The station first joins the lost network again once, 3 s after the loss (`dot11::station`).
  - If that fails, a search begins.
- **Only a remembered network.** That is a network `keystore` keeps a key for (550-NET-0024). With none kept, the station tries nothing and logs nothing.
- **Which one.** The network the station last joined, when it hears it. Otherwise the strongest remembered network it hears. Each try scans first.
- **How often.** A try every 10 s for the first two minutes of a search, then one a minute, until a network is joined.
- **The key.** The station uses the key `keystore` keeps, as `wifi join <name>` with no passphrase does. The station never holds the network's PMK (MC-11.9).

## What the log says

Each try ends in one line:

| Line | Meaning |
|---|---|
| `[WLAN] AUTO-JOIN MIND-TEST: JOINED (TRY 3)` | joined on the third try of this search |
| `[WLAN] AUTO-JOIN: NO REMEMBERED NETWORK HEARD, 1 KEPT (TRY 1), AGAIN IN 10 S` | the scan heard none of the networks `keystore` keeps |
| `[WLAN] AUTO-JOIN MIND-TEST: NO ANSWER (TRY 2), AGAIN IN 10 S` | the join failed, with the station's reason (as `wifi status` gives it) |
| `[WLAN] AUTO-JOIN OFF: NO RDRAND, WHICH A REMEMBERED NETWORK'S HANDSHAKE NEEDS` | said once: without a random source the station refuses WPA2 (issue 103), so it tries nothing by itself |

## How to stop it

- **`wifi leave`** (or `d` in `wifi`'s window):
  - leaves the network;
  - stops the station joining by itself until it next joins a network, by any client's `wifi join`, or until `wlan` starts again.
  - The log says `[WLAN] LEFT THE NETWORK: NO AUTOMATIC JOIN UNTIL THE NEXT ONE`.
- **`forget`** (`f` in `wifi`'s window): the network is no longer remembered. The next try does not choose it.
- **A client's `wifi join`** goes first. It stops a scan the station started by itself, and the station's next try waits 10 s, so the join's outcome stands for that long.
- **No word turns it off for good yet.** A setting kept by `keystore`, a file, or a word of `wifi` are possible later; `wifi`'s part would be the tools track's.

## Who may change the station

- **A client badged `BADGE_WIFI_CHANGE`** may join, leave, forget and set the country. `init` gives one to the shell (`SLOT_WIFI`), and to `link` in developer mode. The shell lends a copy for `REQUEST_WIFI`: to `wifi`, and to `wm`, which passes it to `wifi` from Settings.
- **A client badged `BADGE_WIFI` alone only reads:** `scan`, `status`, the countries, the country and where it came from. Its `join`, `leave`, `forget` and `set-country` answer `denied`, and nothing changes.
- **The station mints that reading client itself** (`wifi.wit` 1.3, `reader`), from its own endpoint, once at its start. Any client of the station may ask for one, since narrowing a client needs nobody's leave. `init` grants its endpoint with the badge right for that (550-KRN-0213). Without that right `reader` answers `unsupported`, and the log says `[WLAN] NO READING CLIENT`.
- **Not done yet:** a launcher lending the reading client to a program that asks only to look. The request bit is there (`REQUEST_WIFI_READ`, 550-KRN-0213); the shell's lending for it is 550-APP-0124. Until then a program gets it only by asking through the changing client it was lent, and dropping that one.

## How it is checked

- `tests/wlan_auto_host.rs` checks:
  - the schedule;
  - how leave and join turn it off and on;
  - the choice: the previous network, the strongest, an unknown signal, a hidden network.
- The `net` suite runs over `wlansim`, on x86 and aarch64:
  - after `keystore` and `wlan` restart, the remembered `MIND-TEST` is joined with no command;
  - the access point goes away for 25 s, and the station's failed tries are logged before it joins again by itself;
  - after `wifi leave`, nothing is joined by itself;
  - `wificheck wifi`, a program lent the changing client, takes the reading one and drops its own; through it the link and the country are read, and `join`, `leave`, `forget` and `set-country` are each denied, the station joined and under its country as before (550-NET-0058).
- These are QEMU runs against a stand-in radio. They say nothing yet about a real radio, such as the MacBook Pro's BCM4331 (MC-12.1).
