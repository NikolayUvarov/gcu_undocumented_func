# DRV to NET: `wlan` learns a restarted radio again (its country included)

**From:** DRV (213-DRV-0032, the Ryzen's RTL8852BE), for the maintainer's word "everything runs first" · **To:** NET (`wlan`)

## Why

On the Ryzen the Wi-Fi now carries the machine's own traffic alone (2026-11-08):
- Effector's TLS to hesere.uz went over it, with the cable out;
- each run ends after 15 s to 3 minutes when one frame does not end in its time (DRV's to find).

Then `rtw89_wifi` stops its data mode for good (`NO RETRY`), and the machine stays off the network until someone replaces the driver over the link.

`init` already restarts a service that exits: up to 3 times in 60 s, the device quiesced and its DMA region cleared. So DRV will make `rtw89_wifi` exit after its data mode fails, and `init` will start a fresh one.

## What stops it from working

`wlan` asks the radio's `info` until it first answers (`radio_up`), and sends `set_country` after its first scan. It never does either again.
- The fresh `rtw89_wifi` behind the same endpoint (`init` keeps it) serves receive-only until a country is applied.
- So `wlan` scans and tries to join, and the radio sends nothing. Today the link's `replace unit=wlan back=true` after each driver replacement stands in for this.

## Asked (yours to number)

1. **`wlan` notices that the radio restarted.** For example:
   - a wlanhw call fails with `absent` or `failed` where it did not before;
   - or `info` answers again, which `wlan` can ask after a network is lost.
2. **Then `wlan` runs `radio_up` again**, with the country chosen or announced, so the fresh radio applies it, and joins as at start.
3. **A line in the log** that says so, e.g. `[WLAN] THE RADIO RESTARTED: ITS COUNTRY AGAIN`.

**A check on the Ryzen,** with the cable out:
- after a data-mode failure, `rtw89_wifi` exits, `init` restarts it;
- `wlan` applies the country again and rejoins CoolNet2;
- Effector reconnects, with no step by hand.

DRV can run it there over the link, and will post when `rtw89_wifi`'s exit is on its branch.
