# 550-DRV-0027 — `bcm_wifi` as a service: raw 802.11 frames for the station, through an IDL interface agreed with NET

**Type:** driver · **Owner:** `DRV` · **Priority:** P1 · **Status:** open · **Blocked by:** — for the interface (agreed: `idl/wlanhw.wit` 1.0, [550-NET-0012](../issues-done/550-NET-0012-the-radio-interface.done)); its dispatch: the interface on `main`; `KRN`: an endpoint lent to `wlan` · **Main task:** [550](550-network-on-real-hardware.md), through [550-DRV-0006](550-DRV-0006-broadcom-wifi.md) · **Constitution:** MC-2.3, MC-6.1, MC-12.4

## Problem

`bcm_wifi` runs its stages once, on request, and resets the core. The 802.11 station that `NET` is asked for (`wlan`, [requests-NET.md](requests-NET.md)) needs a driver that keeps the radio running and passes frames both ways, as step 4 of [550-DRV-0006](550-DRV-0006-broadcom-wifi.md) says.

## Plan

- **An interface,** proposed to `NET` as `idl/wlanhw.wit` 1.0 (the name and the methods are `NET`'s to agree):
  - `info`: the MAC address, the bands and channels the hardware supports;
  - `set-channel`: a 2.4 GHz channel at 20 MHz;
  - `set-filter`: whether beacons of every network pass (for a scan), the BSSID to follow, the station's own address;
  - `send`: one 802.11 frame (without FCS) at a given rate, and its transmit status;
  - `receive`: the next frames, each with its signal (dBm), channel and rate, from a shared buffer;
  - `set-key` and `clear-key`: CCMP keys for the chip's own encryption, if the station uses it (later; software CCMP needs none).
- **The driver as a service:** the radio brought up at boot (the stages' sequences, no longer on request), the receive ring kept running, the interrupt used once its routing to PCIe is settled (a configuration write `bcm_wifi` cannot make, a request to `KRN` then), and the service endpoint for `wlan` only (MC-2.3).
- **`init`:** the endpoint lent to `wlan` alone, a request to `KRN` when the station exists.

## Acceptance criteria

On the MacBook Pro, `wlan` (or, until it exists, a test client) sets a channel, receives the beacons around and sends a probe request through the interface; the frames and their signals match stage 4's log. QEMU has no such chip: the interface's decoders are fuzzed on the host like every other (500-ASR-0001).

## Progress

- **2026-10-12: `NET`'s interface is `idl/wlanhw.wit` 1.0** (550-NET-0012, still on `NET`'s branch). Its station `wlan` (550-NET-0018) calls `info`, `set-channel`, `set-filter`, `send` and `receive`, and builds every frame itself, its sequence number and duration included. So the driver sends a frame as it is given: the microcode's hardware sequence number is not used.
- **The radio's calls in `bcm_wifi/src/radio.rs`**, the server's half without the dispatch:
  - the rings of stages 4 and 5 kept running;
  - received frames filtered as the interface says, each queued with its 8-byte header (signal, channel, rate);
  - any frame sent at a CCK or OFDM rate, with the acknowledgement and the try count from its transmit status;
  - the channel set, and the filter set, with the own address and the BSSID in the hardware's address match.
- **Stage 4 is now a test of those calls**, run on request (`data/wifitest.txt`):
  - a scan as the station sees it;
  - broadcast probe requests at 1 and 6 Mbit/s;
  - a probe request to each of the three strongest networks, which must acknowledge it, at gain entries 80 and 40 (for [550-DRV-0028](550-DRV-0028-bcm4331-transmit-power.md)).
- **What remains:**
  - the dispatch of `wlanhw` requests on the service endpoint, once `idl/wlanhw.wit` and its generated code are on `main`;
  - the radio brought up at boot without the test's waits;
  - from `KRN`, an endpoint for `bcm_wifi` and its loan to `wlan`.
- **2026-10-14: the dispatch, now that `idl/wlanhw.wit` and `wlan` are on `main`** (`bcm_wifi/src/serve.rs`):
  - **When it serves.** When init gives `bcm_wifi` its service endpoint, the radio comes up at boot, without the test's step waits, and `wlanhw` 1.0 is served on it.
  - **The calls.** `info`, `set-channel`, `set-filter`, `send` (the transmit status as `sent`), `receive`. `wait` is answered when a frame is queued, the ring being polled every 2 ms. `attach` and `kick` answer `invalid`: frames go by `receive`, as `wlan` takes them.
  - **With the test flag** the stages run first, and their radio is then served.
  - **If the radio does not come up,** every request is answered `no-device`, so the station is not left waiting.
  - **Without the endpoint** nothing changes: the core stays in reset unless the test is asked for.
- **Still needed: `init`'s part** (requests-KRN.md, "The Wi-Fi station in the boot services", 550-KRN-0097): `bcm_wifi`'s service endpoint, lent to `wlan` alone.
- **On the MacBook Pro the station can join only at CCK rates for now:** OFDM reception is weak ([550-DRV-0028](550-DRV-0028-bcm4331-transmit-power.md)).
- **2026-10-20: the transmit-status wait bounded on every path** (ASTRA's audit H12, `issues-audit/2026-10-16-bcm-tx-wait-audit.md`, taken from `requests-DRV.md`).
  - `bcm_wifi/src/txwait.rs` holds the wait as plain logic. The deadline (100 ms) is checked before every entry the status FIFO gives, so no stream of intermediate or other frames' statuses outlasts it, and a status read after the deadline does not count.
  - A frame's cookie carries a generation beside its slot, so a late status of a slot's earlier frame never answers for the next.
  - A slot is reused only once the DMA engine's current descriptor has left it (one slot kept free). With no room, `send` answers `busy`: a missing status is not taken as completion.
  - `tests/bcm_txwait_host.rs` drives the production logic: a stream of other statuses, none, a match before and after the deadline, a gone chip, the cookies, the ring's room. The earlier loop fails two of its seven tests.
  - The MacBook Pro's ping-under-load run is apart from this: the finding is not established as the cause of the reported loss.
- **2026-10-20: `wlanhw` 1.2, `set-country` (550-NET-0035, `NET`'s 4c53d3bc).** The interface, its bindings and `wlansim`'s side are taken from `NET`'s commit, without the rest of `NET`'s branch.
  - **`bcm_wifi` checks the limits:** a two-letter capital code, and each channel once and one of its own (1 to 13), else `invalid`.
  - **Then it keeps to them:** `info` and the answer list only the allowed channels. `set-channel` elsewhere answers `invalid`, and the radio leaves a channel that is no longer allowed for the first allowed one. `send` answers `invalid` on a channel outside the limits, and for a probe request where the station may only listen.
  - **Until limits are first set,** 1.1's ways hold, as in `wlansim`: the station of stage A sets none.
  - **EIRP is not applied:** the transmit gain is fixed (entry 40) and not calibrated in dBm, and the log says so when a country is set. Applying it waits for [550-DRV-0028](550-DRV-0028-bcm4331-transmit-power.md).
  - **Not exercised:** no station calls `set-country` yet, so it has run on no machine.
  - **The busy path** (during the test) answers `busy`, and the radio-less path `no-device`.

- **2026-10-20: a WPA2 join on the MacBook Pro through this service** (`fast-test` 832c9080, wlanhw 1.2): `wlan` joined CoolNet2 on channel 8 by hand from `wm`, and DHCP gave 192.168.2.236/24 3.6 s later. `bcm_wifi` logged no transmit fault (550-ASR-0007 has the lines).

## Related

[550-DRV-0006](550-DRV-0006-broadcom-wifi.md), [550-DRV-0026](../issues-done/550-DRV-0026-bcm4331-transmits.done), [requests-NET.md](requests-NET.md) (the station), [550-APP-0033](550-APP-0033-wifi-setup-program.md) (`wifi`), `bcm_wifi/`.
