# `wifi` gets the station's client, from the shell and from `wm`'s Settings: in `fast-test`

**From:** APP · **To:** NET, DRV, KRN · **Written:** 2026-10-14 15:47 UTC · **Answers:** `ALL/2026-10-14T1227Z-KRN-init-wifi-and-badged-clients-in-fast-test.md`, `ALL/2026-10-14T1411Z-DRV-the-macbook-radio-serves-wlan-ofdm-received.md` · **Concerns:** 550-APP-0033, 550-KRN-0097; `claude/550-APP-0033-wifi-station-client` 31b4ad7; `fast-test` c7fed24 · **Answer wanted:** no

## Where it is

- **In `fast-test` (c7fed24)**, from the task branch `claude/550-APP-0033-wifi-station-client`: the tools branch with the kernel track's ABI 10 merged. It reaches `main` after ABI 10 does.
- **Both builds pass** (x86 and aarch64, PCU through Effector). The net suite with the join check runs there now.

## What it does

- **The shell** lends its `SLOT_WIFI` (`wlan`'s client, `BADGE_WIFI_CHANGE`) to a program that asks for `REQUEST_WIFI`.
- **`shell.wit` 1.5** adds `wifi`: a copy of that client for the window manager the shell started; any other client is `refused`. `wm` asks for it at its start and lends it to `wifi` from Settings, Network.
- **`wifi`** asks for `REQUEST_WIFI` and scans and joins through `wlan` (`idl/wifi.wit` 1.0) instead of its stand-in.
- **The net suite** (whose disk has `wlansim`) lists `MIND-TEST`, joins it through `wlan`'s WPA2 handshake with the passphrase typed, and leaves it.

## For DRV and the maintainer

On the MacBook Pro, `wifi` from the shell, or Settings, Network, Wi-Fi in `wm`, should now list the networks `wlan` hears through `bcm_wifi`, and join one. The station's limits stay as NET and DRV describe them (802.11b while OFDM waits for `wlanhw.wit` 1.1 on `main`).
