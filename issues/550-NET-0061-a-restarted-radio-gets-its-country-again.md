# 550-NET-0061 — A restarted radio gets its country again, and the station joins by itself

**Type:** network (`wlan`) · **Owner:** `NET` (the kernel session) · **Priority:** P1 · **Status:** open (the station's part done and checked on `wlansim`, below; the Ryzen's check waits for DRV's `rtw89_wifi` exit) · **Blocked by:** for the check on hardware, DRV's `rtw89_wifi` that exits after its data mode fails · **Main task:** [550](550-network-on-real-hardware.md) · **Roadmap:** track D · **Constitution:** MC-7.6, MC-12.1

**Asked by** DRV, 2026-11-08 (`NET/2026-11-08T1840Z-DRV-wlan-learns-a-restarted-radio-again.md` on `internal-dev-messages`), for the maintainer's word "everything runs first": on the Ryzen the Wi-Fi now carries the machine's own traffic, Effector's TLS included.

## Problem

- **On the Ryzen,** a run ends after 15 s to 3 minutes when one frame does not end in its time (213-DRV-0032, DRV's to find). `rtw89_wifi` then stops its data mode for good (`NO RETRY`).
- **`init` already restarts a service that exits,** up to 3 times in 60 s, the device quiesced and its DMA region cleared. DRV makes `rtw89_wifi` exit after such a failure, so a fresh one starts.
- **`wlan` never noticed that.**
  - It asks the radio's `info` until the first answer (`radio_up`).
  - It gives the radio its country once.
  - The fresh radio behind the same endpoint sends nothing until a country is applied, so `wlan` scanned and tried to join, and nothing went out.
  - The link's `replace unit=wlan back=true` stood in for it by hand.

## Plan

- **Notice it.**
  - A fresh radio has no channel set: `wlanhw.wit`'s `info.channel` is 0.
  - While the station is not joined, every 10 s (`RESTART_CHECK_MS`), `wlan` asks a radio it has tuned for its `info`. No channel means it restarted.
- **Then:**
  - the station starts anew over the radio's channels;
  - the radio gets the country the station ran under (with none yet, the country is decided again, as at the first answer);
  - the automatic join joins the remembered network again.
- **The log:** `[WLAN] THE RADIO RESTARTED (IT HAS NO CHANNEL SET): ITS COUNTRY <CC> AGAIN`.

## Acceptance criteria

- **In QEMU:** the network suite kills `wlansim` while the station is joined.
  - `init` starts it again; the station names the restart and gives it its country.
  - The fresh `wlansim` logs that country, and the station joins MIND-TEST by itself.
- **On the Ryzen** (DRV, over the link, with the cable out):
  - after a data-mode failure `rtw89_wifi` exits, and `init` starts it again;
  - `wlan` applies the country and joins CoolNet2 again;
  - Effector reconnects, with no step by hand.

## Progress (2026-11-08)

- **`wlan/src/main.rs`:**
  - `radio_restarted`, between the requests and the automatic join;
  - `tuned`, set by a channel the radio took;
  - `radio_up` split into `station_up` and the country's start.
- **The check in `wifi_join_check`** (the `net` suite), at its end, under RU chosen by the user:
  - MIND-TEST joined with `--remember`, as the automatic join joins only a remembered network;
  - `wlansim` killed and started again by `init`;
  - the line `… ITS COUNTRY RU AGAIN`, the fresh `wlansim`'s `COUNTRY RU`, and `AUTO-JOIN MIND-TEST: JOINED` after it.
- **Passed:** the `net` suite on x86, in the kernel session's container (Ubuntu 24.04.5, QEMU 8.2.2, OVMF, TCG), 2026-11-08. A QEMU run says nothing about the Ryzen's radio (MC-12.1).

## Related

[213-DRV-0032](213-DRV-0032-rtl8852be-wifi.md) (the Ryzen's RTL8852BE), [550-NET-0035](../issues-done/550-NET-0035-the-stations-country.done) (the country), [550-NET-0041](../issues-done/550-NET-0041-wlan-joins-a-remembered-network-by-itself.done) (the automatic join).
