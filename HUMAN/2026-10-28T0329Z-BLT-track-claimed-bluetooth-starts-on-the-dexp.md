# BLT is taken: Bluetooth starts on the DEXP desktop, with its RTL8821C controller

**From:** BLT (the session of DRV, LNK and EFF) · **To:** HUMAN; KRN, DRV, NET, APP informed · **Written:** 2026-10-28 03:29 UTC · **Concerns:** [700](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/main/issues/700-bluetooth.md), TRACKS.md's `BLT` row · **Answer wanted:** only the record in TRACKS.md

## Who and why now

- **The maintainer's word.** On 2026-10-27 at 15:10 UTC the maintainer set the order for the DEXP: Wi-Fi, Ethernet, the screen, then Bluetooth, "as it is needed to connect devices". Bluetooth was to start not before 03:10 UTC today.
- **Where the others stand:**
  - Wi-Fi works on the DEXP (550-DRV-0042, released to its slot B over the cable);
  - Ethernet waits for a cable;
  - the screen waits for `PRT` and `APP` (000-APP-0111).
- **The claim.** Tool: Claude Code. Branch: `claude/BLT-bluetooth`. Tasks from 700-BLT-0001.

## The first stage: the controller up on the DEXP

The DEXP's controller is the RTL8821CE's Bluetooth half, `0BDA:C821`, at full speed on the xHCI's port 1. `usb_host` already enumerates it, and no class driver takes it.

1. **700-DRV-0043** (DRV, mine):
   - `usb_host` hands the wireless controller class (0xE0/01/01) to a new badge, `BADGE_BLUETOOTH`;
   - it takes that client's HCI commands, which go to the device as a whole as class requests (0x20/0x00);
   - this is `usb.wit` 1.4.
2. **700-BLT-0001: `bt_hci`.**
   - The HCI transport: commands, events reassembled from the interrupt endpoint's packets, ACL on the bulk pair.
   - Realtek's patch and configuration loaded before use.
   - Proprietary files: `rtl_bt/rtl8821c_fw.bin` and `rtl8821c_config.bin` from the pinned linux-firmware revision, "Redistributable", `LICENCE.rtlwifi_firmware.txt`. They are fetched by a script and put on the DEXP over the cable (800-LNK-0027), never committed.
   - The loader is written from the patch file's format, not adapted from Linux's `btrtl.c`, which is GPL.
3. **KRN** (asked apart): the boot image `bt_hci` and `init`'s grant. This is an ABI step.
4. **First evidence on the DEXP:** the controller's version before and after the patch, its address, and an LE scan that hears the advertisers around.

After that come the host stack (HCI, L2CAP, ATT/GATT, SMP), HID for keyboards and mice, the tool `bt`, and link keys in `keystore` (NET). Each is split from 700 as it starts.
