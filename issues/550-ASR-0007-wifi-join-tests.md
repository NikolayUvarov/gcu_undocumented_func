# 550-ASR-0007 — Joining a Wi-Fi network, tested: in QEMU on every gate, and on the MacBook Pro on request

**Type:** assurance · **Owner:** `ASR` · **Priority:** P1 · **Status:** in progress · **Blocked by:** — (APP's join without a screen and start script, and NET's `net` suite join check, are in `main`) · **Main task:** [550](550-network-on-real-hardware.md) · **Constitution:** MC-12.1, MC-12.2, MC-12.9

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

## Progress

- **2026-10-14: the tools track's two parts are planned** as 550-APP-0078 (`wifi join`, `wifi status`) and 000-APP-0079 (the start script). It proposed the boot volume's root for `autorun.msh` instead of `data/`: a script under `data/`, where programs may write, would run at every boot with the shell's authority (MC-3.11). Taken: `build-and-write.sh --wifijoin` writes the root's `autorun.msh`.
- **`tests/hardware/wifijoin.msh`** (on `claude/ASR-DRV` and `fast-test` until its gate):
  - it reads the network's name from `data/wifijoin.txt`;
  - it waits up to 60 s for the station, then runs `wifi join "<name>" --passphrase-file data/wifijoin.txt`;
  - after a join it waits up to 30 s for a card's address and pings its gateway;
  - it notes each step on the screen and in the system log, and ends with `*** WIFI JOIN Complete. Data collected ***`.
  
  Its `requires:` is `file wifi network log`. It is written against 550-APP-0078's planned lines: until that is in an image, `wifi join` fails and the script logs why.

- **2026-10-20: both of APP's parts and the QEMU half are in `main`.**
  - APP's `wifi join --passphrase-file` (550-APP-0078) and the root's `autorun.msh` (000-APP-0079) are in `main`. `tests/hardware/wifijoin.msh` uses them as they are: the command line, `JOINED`, and the exit status.
  - NET's `net` suite checks over `wlansim`: the join, a wrong passphrase refused, the lease 10.0.4.15 and a ping of 10.0.4.1, 802.11b's rates alone, and a remembered network joined again. That is this task's QEMU half.
  - **Left:** the MacBook Pro's run. `build-and-write.sh --wifijoin` writes `data/wifijoin.txt` and `autorun.msh` after the image, and the log ends with `*** WIFI JOIN Complete. Data collected ***`.

- **2026-10-20: the MacBook Pro joined CoolNet2** (`fast-test` 832c9080, the Wi-Fi firmware copied after the write). The maintainer joined by hand from `wm`'s Settings, Wi-Fi; the log was read over MIND Link (800-LNK-0003):
  - `[WLAN] JOINING CoolNet2 ON CHANNEL 8` at 256.782 s, `Joined` at 256.823 s (WPA2), the network kept by `keystore`;
  - `[NETSTACK] DHCP 192.168.2.236/24 GATEWAY 192.168.2.1 DNS 8.8.8.8 ON CARD 2` at 260.434 s, and the default route on it;
  - no transmit fault from `bcm_wifi` in the log, and the passphrase nowhere in it.
- **Left:** the scripted run (`tests/hardware/wifijoin.msh` as `autorun.msh`, `build-and-write.sh --wifijoin`), with its ping of the gateway and its end marker.

## Related

[550-DRV-0027](550-DRV-0027-bcm4331-frame-service.md), [550-DRV-0029](550-DRV-0029-bcm4331-receives-ofdm.md), [550-APP-0033](550-APP-0033-wifi-setup-program.md), the requests to `APP` and `NET` on `internal-dev-messages` (2026-10-14), `tests/wlansim/`.
