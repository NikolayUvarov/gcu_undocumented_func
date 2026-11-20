# RTL8852B Bluetooth: Realtek's patch in the "RTBTCore" format

What `bt_hci` (700-BLT-0004) relies on to patch the Bluetooth half of the Ryzen's RTL8852BE. The transport and the vendor commands are the RTL8821C's ([rtl8821c-bt.md](rtl8821c-bt.md)); only the controller, the file and its layout differ.

## The controller

- **USB `0BDA:B85B`,** full speed. On the Ryzen it is on the first xHCI's root port 3.
- **Its ROM** answers Read Local Version Information with manufacturer 0x005D, LMP subversion 0x8852 and HCI revision 0x000B.
- **After the patch** the same command gives LMP subversion 0x4E04 and HCI revision 0x42D3 (the Ryzen, 2026-11-20).

## The file

`rtl_bt/rtl8852bu_fw.bin` is 127,301 bytes, SHA-256 `fa1a98b9…4cba`. It comes from linux-firmware revision `599764611a8ac213c6aa6dad17c941c2f46b53cb`, the RTL8852BE's and the RTL8821C's.
- **The licence.** WHENCE's `rtl_bt` section says: "Redistributable. See LICENCE.rtlwifi_firmware.txt for details."
- **Where it lives.** [`scripts/firmware/rtl8852b_bt.py`](../scripts/firmware/rtl8852b_bt.py) fetches it into the ignored `proprietary/firmware/rtl_bt/`. `scripts/proprietary.sh` puts it on a disk's `data/firmware/rtl_bt/`, or over the link.
- **No configuration file.** linux-firmware has none for this controller, and the patch is complete without one.

## The layout

Read from the file itself; all little-endian.

| Bytes | What |
|---|---|
| 0–7 | the signature `RTBTCore` |
| 8–15 | the version (here `f1 01 35 01 c4 33 02 00`) |
| 16–19 | the number of sections (here 2) |
| then | the sections, each: an opcode (4 bytes), a length (4 bytes), then that many bytes |
| the last bytes | the extension, ending in the signature `51 04 FD 77`, read backwards as in the "Realtech" format; its project ID is 20 |

**A patch section** (opcode 1) holds:
- a count (2 bytes);
- 2 reserved bytes;
- that many parts, each:
  - an ECO byte, which is the ROM version plus one;
  - a priority byte;
  - 2 more bytes;
  - a length (4 bytes);
  - then the part's bytes.

**What the pinned file holds:**
- the patch section, 127,254 bytes, with two parts of priority 15:
  - ECO 1 (ROM version 0): 67,133 bytes;
  - ECO 2 (ROM version 1): 60,101 bytes;
- an empty section with opcode 8;
- the extension, in the file's last 11 bytes.

**What is downloaded:**
- the parts whose ECO is the ROM version plus one;
- taken from the patch and dummy sections;
- in rising priority;
- nothing else: no version is written into them and no configuration follows.

A file with a security section (opcode 3) is refused, and so are two chosen parts of the same priority. The pinned file has neither.

**The download** uses the RTL8821C's commands (`0xFC6D`, `0xFC20`). The ROM-0 image is 67,133 bytes, or 267 fragments. The index runs 0 to 0x7F, then 1 to 0x7F over and over. So the 256th fragment is 1, not 0x80. Before 700-BLT-0004, `bt_hci` gave it 0x80, the last fragment's mark. The RTL8821C's 217 fragments never reached it.

**How it was written.**
- **The layout** above was read from the file's bytes.
- **Linux's `btrtl.c` (GPL-2.0)** was read for facts the bytes do not show: that opcode 3 is a security section needing a key the controller gives, and that opcode 2's parts are patch bytes too.
- **No code** was taken from it. `bt_hci/src/rtl.rs` is written from this description.

## On the Ryzen (2026-11-20)

- **The patch.** `PATCH DOWNLOADED IN 555 MS`, then `RTL8852B PATCHED: LMP SUBVERSION 4E04, HCI REVISION 42D3`.
- **The scans.**
  - In the rounds of 2026-11-20 before the patch, the ROM's scans heard nothing: 0 LE and 0 BR/EDR devices.
  - The patched controller heard 9 LE devices in its first LE scan and 3 BR/EDR devices in its first inquiry.
- **The coexistence.** The Wi-Fi firmware now passes the controller's BT_INFO reports, which it never did unpatched.
  - The first report said the controller ignored the WLAN; `rtw89_wifi` told it not to, as Linux does.
  - The inquiry shows as INQ/PAGE.
