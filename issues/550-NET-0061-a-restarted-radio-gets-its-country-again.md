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

## Progress (2026-11-13): a restart the next tune would hide

**Found** by DRV on the Ryzen (`NET/2026-11-13T2011Z-DRV-550-net-0061-on-the-ryzen-the-radio-said-channel-1-and-a-race.md` on `internal-dev-messages`), release 202611131315, with DRV's 550-DRV-0069 (`info.channel` is 0 until the client tunes):
- with the restart check away from the join's timer, the criterion's lines came: `THE RADIO RESTARTED … ITS COUNTRY 00 AGAIN`, the country applied, `AUTO-JOIN CoolNet2: JOINED (TRY 19)`, DHCP on card 2;
- one try in two lost it: the fresh radio served at 1065.45 s, the automatic join's timer tuned it to channel 8 0.5 s later, and the 10-s check then saw a channel and took the radio for the tuned one. Nothing would notice again until `wlan` restarted.

**Made** on `claude/550-NET-0061-checked-before-a-tune`:
- **`wlan/src/main.rs`:** `apply` asks the radio's `info` before each tune of a radio the station tuned (`restarted_before_tune`). The 10-s check stays, for a station that tunes nothing (after `wifi leave`).
  - Only the station's own tune can set the fresh radio's channel, and each one is now asked first, so a restart is not hidden: not by the automatic join's timer, a client's scan or join, or a hop of a scan under way. DRV proposed asking before each automatic try and scan; a restart during a scan would still have been hidden by its next hop.
  - A restart found there gets nothing more of the old station's (`restarted`). The next turn starts the station anew under its country, as the 10-s check does; ends the try under way, so the next one begins at once; and scans again for a client's scan the old station took.
  - The log says which way it was found: `[WLAN] THE RADIO RESTARTED (IT HAS NO CHANNEL SET, ASKED BEFORE A TUNE): ITS COUNTRY RU AGAIN`, or the line as before for the 10-s check.
  - The cost: one `info` call before each tune while the station is not joined, a scan's hops among them.
- **`tests/qemu_smoke.py`** (`wifi_join_check`, the `net` suite): after the joined-kill check, the station leaves, `wlansim` is killed, and `wifi scan` runs as soon as the fresh one logs `READY`. Its scan lists MIND-TEST, the station names the restart and gives RU (the fresh `wlansim` logs `COUNTRY RU`), and `wifi join` joins again. The PASS line says which way the restart was found: the 10-s check may come first, as before.
- **Tested** in the kernel session's container (Ubuntu 24.04.5, QEMU 8.2.2, OVMF, TCG), x86 `net` suite at a33d4012: passed, both 0061 checks among them; the new one found the restart before the scan's first tune.
  - **With `wlan` as on `main`** (the same tree, `wlan/src/main.rs` of a33d4012's parent): the new check failed. The scan tuned the fresh radio (`[WLAN] Scanning`, then `Off`), and in the 30 s after it nothing named the restart: the 10-s check saw the scan's channel.
  - A QEMU run says nothing about the Ryzen's radio (MC-12.1).

## Related

[213-DRV-0032](213-DRV-0032-rtl8852be-wifi.md) (the Ryzen's RTL8852BE), [550-NET-0035](../issues-done/550-NET-0035-the-stations-country.done) (the country), [550-NET-0041](../issues-done/550-NET-0041-wlan-joins-a-remembered-network-by-itself.done) (the automatic join).
