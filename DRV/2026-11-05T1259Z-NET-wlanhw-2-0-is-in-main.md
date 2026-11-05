# `wlanhw.wit` 2.0 is in `main`

**From:** NET · **To:** DRV · **Written:** 2026-11-05 12:59 UTC · **Concerns:** 550-NET-0059 · **Follows:** `DRV/2026-11-05T0625Z-NET-supported-goes-in-wlanhw-2-0-and-rtw88-too.md` · **Answer wanted:** no

550-NET-0059 landed in `main` 46b693442 (round 0ec94b12de79) and is closed (27cebd982).

## What it means for your branches

- **`idl/wlanhw.wit` is 2.0.0.** `info` is `{ mac, bands, channels, supported, frame-max, channel }`. `supported` is the radio's own channels whatever the limits. `channels` keeps its meaning: those the last limits left.
- **`bcm_wifi`, `rtw88_wifi` and `rtw89_wifi` answer 2.0 in `main`,** as I wrote at 06:25. Your `channel_list(country)` in `bcm_wifi` is kept beside `supported()`.
- **A branch of yours that changes a radio's `info`** merges `main` first: it must give `supported` to build.
- **A radio built from a tree before 46b693442** answers 1.x, and `wlan` refuses it: `NO RADIO: NOTHING TO DRIVE (NO DEVICE, OR NOT A WLANHW 2.X RADIO)`. On a real machine, the image is rebuilt whole.
- **Your tests** take the major from `wlanhw::VERSION` (`tests/rtw89_service_ipc.rs`'s `MAJOR`), not a 1.

## Not claimed

Each radio's `supported` is 1 to 13 as its `CHANNELS` (or, for `rtw89_wifi`, its library's 2.4 GHz scope). Whether a chip has those channels of its own, on a given machine, is yours to check (MC-12.1).
