# 550-ASR-0007 — Joining a Wi-Fi network, tested: in QEMU on every gate, and on the MacBook Pro on request

**Type:** assurance · **Owner:** `ASR` · **Priority:** P1 · **Status:** open · **Blocked by:** `APP`: a join without a screen and a start script ([requests-APP.md](requests-APP.md)); `NET`: the `net` suite's join check · **Main task:** [550](550-network-on-real-hardware.md) · **Constitution:** MC-12.1, MC-12.2, MC-12.9

## Problem

On the MacBook Pro the radio (`bcm_wifi`, 550-DRV-0027, 550-DRV-0029), the station (`wlan`) and the tool (`wifi`) now run together. Nothing checks that a network can be joined, though:

- **In QEMU** the station runs over `wlansim`, a stand-in radio with a WPA2 access point behind it. No suite joins it yet.
- **On the MacBook Pro** a join is tried by hand, typing the passphrase into `wifi`. On 2026-10-14 the tools track saw a join with a passphrase fail in QEMU without a visible reason.

The maintainer asked for the join to be tested without typing (2026-10-14).

## Plan

1. **QEMU, on every gate** (the network track's part, planned in 550-NET-0018):
   - in the `net` suite, with `wlansim` on the disk, `wlan` joins `MIND-TEST` with its known passphrase and gets 10.0.4.15;
   - `ping` reaches the gateway;
   - a wrong passphrase is refused with its reason.

   Driving `wifi` from the keyboard (the tools track's suites) checks the tool's part too.
2. **The MacBook Pro, on request** (this task, with the tools track's two parts):
   - **The network and its passphrase.** `build-and-write.sh --wifijoin` puts `data/wifijoin.txt` (the network's name, then the passphrase) on the written disk's boot volume. It takes them from the maintainer's own file outside the repository, as the proprietary files and `data/wifitest.txt` go. They are never in the repository, an image, a message or a log.
   - **The start.** At start the shell runs `data/autorun.msh` when it exists (the tools track's part). The script comes from `tests/hardware/wifijoin.msh` (this task), and `build-and-write.sh --wifijoin` puts it on the disk.
   - **The join.** The script joins with `wifi join` without a screen, the passphrase read from the file (the tools track's part). It logs the result, or the station's reason for a refusal, then `ip`'s address and a `ping` of the gateway.
   - **The end.** It ends with `*** WIFI JOIN Complete. Data collected ***`.
3. **The evidence:** each run is recorded in this issue with its configuration (MC-12.1, MC-12.9): the access point, its channel and signal, 802.11b or g.

## Acceptance criteria

- **QEMU.** The `net` suite joins `MIND-TEST` and gets its address on every gate run, and refuses a wrong passphrase with its reason.
- **MacBook Pro.** With `--wifijoin` and no typing, one boot logs a WPA2 join, the address DHCP gave, and a gateway that answers `ping`.
- **The passphrase** appears in no log line, commit or message. A test only reads it from `data/wifijoin.txt`, a file on the maintainer's own disk, kept in plain text as the proprietary files are kept (AGENTS.md, section 3).

## Related

[550-DRV-0027](550-DRV-0027-bcm4331-frame-service.md), [550-DRV-0029](550-DRV-0029-bcm4331-receives-ofdm.md), [550-APP-0033](550-APP-0033-wifi-setup-program.md), requests-APP.md, requests-NET.md, `tests/wlansim/`.
