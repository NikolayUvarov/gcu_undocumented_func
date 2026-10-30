# 700-BLT-0001 — `bt_hci`: the HCI over USB, and the DEXP's RTL8821C patched and listening

**Type:** driver · **Owner:** `BLT` · **Priority:** P1 (the maintainer's order for the DEXP, 2026-10-27) · **Status:** in progress (2026-10-28) · **Blocked by:** `KRN`: the boot image `bt_hci` and `init`'s grant (asked on `internal-dev-messages`, 2026-10-28); `DRV`: 700-DRV-0043 (`usb_host`'s badge) · **Main task:** [700](700-bluetooth.md) · **Constitution:** MC-3.3, MC-10.2, MC-11.4, MC-12.1, MC-12.3

## Problem

The maintainer's order for the DEXP desktop (2026-10-27) is Wi-Fi, Ethernet, the screen, then Bluetooth, "as it is needed to connect devices", from 03:10 UTC on 2026-10-28.

- **The controller.** It is the RTL8821CE's Bluetooth half, `0BDA:C821`, on the DEXP's xHCI.
- **No class driver takes it.** `usb_host` enumerates it.
- **Realtek's ROM needs a patch** from two proprietary files before the controller is of use.

## Plan

1. **`bt_hci/src/hci.rs`:**
   - HCI command packets;
   - events joined from the interrupt endpoint's packets;
   - Command Complete and Command Status;
   - the version, the address;
   - the devices heard, from LE advertising reports, extended inquiry results and inquiry results with RSSI, with their names, classes and appearances.
2. **`bt_hci/src/rtl.rs`:** the patch made from the files ([hwdocs/rtl8821c-bt.md](../hwdocs/rtl8821c-bt.md)):
   - the "Realtech" file's head and its project ID;
   - the patch for the ROM version, with the file's version over its last four bytes;
   - the configuration appended;
   - 252-byte fragments with their index bytes.

   It is written from the files' layout, not from Linux's GPL `btrtl.c`.
3. **The service, `bt_hci/src/main.rs`:**
   - it claims the controller `usb_host` gives its badge;
   - it resets the controller and reads its version;
   - an unpatched RTL8821C gets its patch from `data/firmware/rtl_bt/`, checked by size and SHA-256, then a reset;
   - it reads the address and sets the event mask and the inquiry mode;
   - at start and every five minutes it runs an active LE scan of 10 s and a general inquiry of 10.24 s. Each device heard is logged once a round: its address, signal, name, and class or appearance.
4. **The files:**
   - `scripts/firmware/rtl8821c_bt.py` fetches and pins them, and `scripts/proprietary.sh fetch` runs it;
   - they reach the DEXP by `scripts/proprietary.sh link effector485` (800-LNK-0027).
5. **`KRN` and `DRV`:** the boot image and its grant, and `usb_host`'s badge (700-DRV-0043).

The stack comes next, as its own tasks: `bt_host`, L2CAP, ATT/GATT, SMP, HID, the tool `bt`, and link keys in `keystore`.

## Progress

- **2026-10-28:** written.
  - `bt_hci` builds for x86 without warnings.
  - The host tests (`tests/bt_hci_host.rs`, 6) cover:
    - command packets;
    - events joined across 16-byte packets, and bytes past an event's end dropped;
    - Read Local Version and the address;
    - LE reports, extended inquiry results, malformed advertising data;
    - the patch chosen by the ROM version, with the file's version and the configuration;
    - the fragments' indices past 0x7F;
    - the real RTL8821C files when fetched: project 10, version `0x75B8F098`, the two patches' lengths.
  - The fetcher pins the files.

## Acceptance criteria

- [x] Host tests of the HCI encodings and the patch's making (`tests/bt_hci_host.rs`).
- [ ] On the DEXP:
  - `bt_hci` claims `0BDA:C821` and logs the ROM's version;
  - the patch is downloaded, and the version after the reset differs;
  - it logs the address;
  - a round hears the advertisers and discoverable devices around.

  Evidence for the DEXP only (MC-12.1).
- [ ] The `usb`, `keys` and `link` QEMU suites pass with the boot image, where QEMU has no controller (`bt_hci` waits, then says none).

## Progress (2026-10-31)

**The DEXP's criterion is met with services not yet in a release.**
- **The setting:** release 202610301115 (ABI 25), with `rtw88_wifi` (DRV's 700-DRV-0046) and `bt_hci` (700-BLT-0002) put by `effector485 replace`, unverified, built from that release's tree.
- **The log:**
  - `CONTROLLER 0BDA:C821`;
  - the ROM's `REVISION 000C … SUBVERSION 8821`;
  - `PATCH 75B8F098 … 34926 BYTES`, then `PATCHED: LMP SUBVERSION F098` (2026-10-30);
  - `ADDRESS 1C:79:2D:F0:6B:A6`.
- **A round heard 15 LE devices and a BR/EDR one** (`CLASS 0C043C AUDIO`). Later ones heard named devices: `"BRAVIA 4K GB"`, `"[TV] Samsung AU7100 65 TV"`.
- **Why it heard nothing before:** `rtw88_wifi` granted the air to the Wi-Fi alone (700-DRV-0046).
- **What closes this task:** a release that carries both, run on the DEXP. The QEMU criterion is also still to record.

## Related

[700](700-bluetooth.md), `700-DRV-0043`, `800-LNK-0027`, `550-DRV-0042` (the same card's Wi-Fi).
