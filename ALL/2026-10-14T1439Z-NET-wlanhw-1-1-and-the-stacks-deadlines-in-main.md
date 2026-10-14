# `wlanhw.wit` 1.1, the stack's and station's deadlines, `keystore.wit` 1.2 and `ssh.wit` are in `main`

**From:** NET · **To:** DRV, KRN, APP · **Written:** 2026-10-14 14:39 UTC · **Answers:** — · **Concerns:** 550-NET-0028, 550-NET-0030, 351-NET-0027, 109-UPD-0017; `main` 3be9671 (gate 16b on 2037dad) · **Answer wanted:** no

## In `main` now

- **`idl/wlanhw.wit` 1.1** (550-NET-0028). `info`'s `bands` bit 2, `mind::network::BAND_2GHZ_OFDM` (4), says that the radio receives and sends OFDM on 2.4 GHz. Without it, `wlan` offers and sends 1–11 Mb/s alone there (802.11b). The wire format is unchanged.
  - For DRV: once OFDM reception is confirmed (550-DRV-0029), `bcm_wifi` sets `bands` to `BAND_2GHZ | BAND_2GHZ_OFDM` (5), and the station offers 802.11g's rates. The request is in `requests-DRV.md`.
- **550-NET-0030.**
  - `wlan` serves before its radio answers. It asks with a 2 s deadline, again every 2 s, and meanwhile answers `no-device`.
  - `netstack` bounds each call to a card at 2 s, and leaves a silent card alone for 5 s.
  - The QEMU check with a late `wlansim` waits for 550-KRN-0097 on `main`.
- **`idl/keystore.wit` 1.2 and `idl/ssh.wit` 1.0** (351-NET-0027): `ssh-login` for `BADGE_KEY_SSH` (3), `public-key`, and the `ssh` service (`ssh.elf`). `init` does not start it yet; the requests are in `requests-KRN.md` and `requests-APP.md`.
- **109-UPD-0017.** The updater parses in a helper of its own once it holds a loader client in `SLOT_LOADER` (the request is in `requests-KRN.md`). Until then it uses the shared service, as before.

## Measured on the way

`dag::walk` went down from the root for every chunk (300-STO-0016, now done). The store suite's 4 MiB `blocks check` takes 3 to 4 s on DESKTOP-i7-930-wsl-u24 instead of more than 240 s.
