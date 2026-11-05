# Ready: where the Wi-Fi country came from (550-APP-0082), and the machine's name with `hostname` (000-APP-0123)

**From:** APP · **To:** LANDING · **Written:** 2026-11-05 15:08 UTC · **Concerns:** 550-APP-0082, 000-APP-0123

**Ready:** `claude/wizardly-franklin-kec1a9` d06de5cb3a7ab96d1db828fde94767a1c877644a

It contains `main` 426202048.

## What it brings

- **550-APP-0082** (0fd9a5e7e): `wifi.wit` 1.2's `country-origin`, shown so that no country reads as silence (the maintainer's decision, 2026-11-04, by way of NET).
  - `wifi country`'s line now gives the reason, in `wlan`'s words.
  - The screen's line and its `ORIGIN=` field.
  - `wm`'s Settings, Network shows the station's country and why, read through `wm`'s copy of the station client. It only reads.
  - NET's three `wifi country` checks in the `net` suite now expect the new wording.
- **000-APP-0123** (d06de5cb3): `hostname [name]` in the shell.
  - The machine's name is kept in `data/machine/name`, the file 000-KRN-0180 reads, and it is `mindcore` when unset.
  - A program's client is asked first; the link's client in developer mode sets it at once.
  - EFF's 750-EFF-0018, NET's 000-NET-0050 and LNK's 800-LNK-0028 wait for it, by way of KRN.

## Tested on this container's QEMU x86, 4 CPUs, TCG

- **`tools`, `wm` and `net`:** passed at 0fd9a5e7e.
  - `net`'s first run there failed at `wifi_boot_check`, because this container's `wlansim` fixture predated `wlanhw.wit` 2.0: `wlan` logged `NO RADIO … NOT A WLANHW 2.X RADIO` and was quarantined.
  - With the fixtures rebuilt as `scripts/x86_fixtures.sh` does, it passed: 33 checks, among them `wm`'s network page naming UZ over `wlansim`.
- **`shell`:** passed at d06de5cb3, with the new `hostname_check`.
- **Host:** `tests/wifi_host.rs` (20), `tests/wm_host.rs` (47), `tests/shell_host.rs` (27).
- **Not run here:** aarch64, and the other groups. The change is in `wifi`, `wm` and the shell, none of it architecture-specific.
