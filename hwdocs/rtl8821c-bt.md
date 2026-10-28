# RTL8821C Bluetooth: the HCI over USB and Realtek's patch

What `bt_hci` (700-BLT-0001) relies on for the RTL8821CE's Bluetooth half, the DEXP desktop's controller. It covers the transport and the patch, not the stack.

## The controller

- **USB `0BDA:C821`,** full speed. The DEXP's xHCI enumerates it on port 1.
- **Its HCI interface** is class 0xE0, subclass 1, protocol 1 (Bluetooth Core 5.3, Vol 4 Part B, 2.1):
  - events come on the interrupt IN endpoint, in packets of up to 16 bytes, one event a transfer;
  - ACL data use the bulk pair.
  - Interface 1 holds the isochronous endpoints for SCO, which `bt_hci` does not use yet.
- **Commands** go to the device in a control transfer (Vol 4 Part B, 2.2): request type 0x20 (class, host to device, the device), request 0, the command packet as the data stage. `usb_host` takes this from the client of a Bluetooth interface alone (700-DRV-0043).
- **Its ROM** answers Read Local Version Information with:
  - manufacturer 0x005D (Realtek);
  - LMP subversion 0x8821;
  - HCI revision 0x000C.

## The patch

**The files.** `rtl_bt/rtl8821c_fw.bin` (54,636 bytes, SHA-256 `dfe7ef77…743c`) and `rtl_bt/rtl8821c_config.bin` (10 bytes, SHA-256 `292717b0…e382`) come from linux-firmware revision `599764611a8ac213c6aa6dad17c941c2f46b53cb`, the RTL8852BE's. WHENCE's `rtl_bt` section says: "Redistributable. See LICENCE.rtlwifi_firmware.txt for details". `scripts/firmware/rtl8821c_bt.py` fetches and pins them into `proprietary/firmware/rtl_bt/`. `scripts/proprietary.sh` copies them onto a written disk, or `link` puts them over the cable (800-LNK-0027), under `data/firmware/rtl_bt/`. They are never committed and never in an image (AGENTS.md, section 3).

**The patch file's layout,** read from the file itself (all little-endian):

| Bytes | What |
|---|---|
| 0–7 | the signature `Realtech` |
| 8–11 | the firmware's version (here `0x75B8F098`) |
| 12–13 | the number of patches *n* (here 2) |
| then | *n* chip IDs (2 bytes each), *n* lengths (2 bytes), *n* offsets (4 bytes) |
| last 4 | the extension's signature `51 04 FD 77` |

**The extension.** Its entries are read backwards from before the signature: an opcode, a length, then that many bytes before them. Opcode 0 is the project ID, here 10 for the RTL8821C, and 0xFF ends the entries.

**The configuration file** starts with `55 AB 23 87`, then a 2-byte length and its entries.

**What is downloaded:**
- the patch whose chip ID is the ROM version plus one;
- the file's version written over that patch's last four bytes;
- then the whole configuration file.

**Vendor commands:**
- `0xFC6D` returns the ROM version after the status;
- `0xFC20` takes an index byte and up to 252 bytes of the image. The index runs from 0 to 0x7F, then again from 1, and the last fragment has bit 7 set. The fragments are the image's whole 252-byte pieces and one more, which holds the rest and may be empty;
- each is answered by a Command Complete;
- a reset follows.

The two patches in the file are for chip IDs 1 and 2: 19,476 and 34,916 bytes.

**How it was written.** This description and `bt_hci/src/rtl.rs` were written from the files' bytes and from the transport's public description. No code was taken from Linux's `btrtl.c`, which is GPL. What the controller does with the result is the DEXP's to show (MC-12.1).

## What is not done

- SCO's isochronous endpoints.
- The configuration's UART entries, which a USB controller does not use.
- Other Realtek chips: their IDs and project numbers are not listed, and `bt_hci` patches the RTL8821C alone.
