# 550-DRV-0042 — `rtw88_wifi`: the DEXP's RTL8821CE Wi-Fi as a `wlanhw` radio

**Type:** driver · **Owner:** `DRV` · **Priority:** P1 (the maintainer's first, 2026-10-27) · **Status:** in progress (2026-10-27) · **Blocked by:** `KRN`: a boot image and `init`'s grant for `10ec:c821` (announced on `internal-dev-messages`, 2026-10-27; to be asked with the image's name and DMA size) · **Main task:** [550](550-network-on-real-hardware.md) · **Constitution:** MC-3.3, MC-10.2, MC-12.1, MC-12.3

## Problem

The maintainer's DEXP desktop has a Wi-Fi card at PCI 03:00.0 and no driver for it (`rtw89_wifi NOT STARTED: NO DEVICE`).

- **The card is most likely an RTL8821CE** (`10ec:c821`, Linux `rtw88`):
  - its Bluetooth half is on USB as `0BDA:C821`;
  - its BARs are those of an `rtw88` chip: 256 bytes of I/O, then 64 KiB of memory.

  The PCI ID itself is not yet read: `stat devices` shows none (asked of `KRN`).
- **No other way onto the network.** No Ethernet cable is at hand.
- **The settings are on the disk.** The Wi-Fi network was kept with `--keep-settings`, so `wlan` joins by itself once a radio serves `wlanhw`. The server's Effector agent then connects, and the maintainer activates it.

The maintainer's order (2026-10-27): Wi-Fi, then Ethernet, then the screen, then Bluetooth.

## Plan

A new boot service, `rtw88_wifi`, ported from Linux v6.12's `rtw88` (`adc218676eef`) under its BSD-3-Clause option. It serves `wlanhw` as `bcm_wifi` does:
- raw 802.11 frames each way;
- the channel;
- the receive filter;
- a frame's tries and whether it was acknowledged.

`wlan`, `dot11` and `wpa` do the rest in software.

1. **The tables** (done):
   - `scripts/hardware/rtw88_tables.py` generates `rtw88_wifi/src/tables.rs` from the pinned `rtw8821c_table.c`: MAC, BB, AGC, AGC-BTG, RF A, power by rate, power limits;
   - every entry is kept, with its condition words;
   - `cond.rs` evaluates them at run time against the chip's cut and the board's RFE type, as `phy.c` does;
   - `load.rs` writes them, with their delays.
2. **The firmware** (done): `scripts/firmware/rtl8821ce.py` fetches `rtw88/rtw8821c_fw.bin` (139,472 bytes, SHA-256 `2ef409bc…f3f0`) from the linux-firmware revision the RTL8852BE's comes from. `scripts/proprietary.sh fetch` runs it, and `copy` puts it under `data/firmware/rtw88/`. Its terms: "Redistributable", `LICENCE.rtlwifi_firmware.txt`.
3. **Power on, MAC set-up, the eFuse** (MAC address, RFE type, crystal, TX power), and the firmware download.
4. **The PHY and the channel:**
   - the tables;
   - the crystal;
   - the antenna for Wi-Fi alone, as the Bluetooth half shares it;
   - 2.4 GHz channels at 20 MHz first, 5 GHz after;
   - a TX power from the eFuse.
5. **PCIe rings:** a management queue, a data queue and the receive ring, polled.
6. **TX and RX:**
   - descriptors with a fixed rate, a retry limit and a report of the acknowledgement;
   - the receive descriptor and the frame's signal;
   - firmware messages told apart from frames.
7. **The service:** `wlanhw`'s calls over the above.
8. **`KRN`:** the boot image `rtw88_wifi` and `init`'s grant:
   - the device;
   - BAR2 without mastering until the driver resumes it, as for `rtw89_wifi`;
   - a DMA region;
   - a reader of `data/firmware/`.
9. **On the DEXP:**
   - the driver starts, reads the eFuse and runs the firmware;
   - a scan hears the networks;
   - `wlan` joins the kept network, DHCP gives an address, the Effector agent connects.

## Acceptance criteria

- [ ] Host tests: the tables and their conditions, the eFuse decoding, the firmware header, the descriptors.
- [ ] On the DEXP:
  - the driver logs the chip, its MAC address and the firmware's version;
  - `wifi scan` lists the networks around;
  - `wlan` joins the kept network, and `ip` shows an address;
  - the server's Effector agent connects.

  Evidence for the DEXP only (MC-12.1).
- [ ] `hwdocs/rtl8821ce.md`: the sources, hashes and what was run. `THIRD_PARTY.md` and `LICENSES/BSD-3-Clause-rtw88.txt` record the adaptation.

## Related

[213-DRV-0032](213-DRV-0032-rtl8852be-wifi.md) (`rtw89_wifi`), [550-DRV-0006](550-DRV-0006-broadcom-wifi.md) (`bcm_wifi`), [211-DRV-0041](211-DRV-0041-the-dexps-rtl8168h.md), [211-DRV-0040](../issues-done/211-DRV-0040-more-scratchpad-pages.done).
