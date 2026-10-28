# Ready: the DEXP's Wi-Fi driver `rtw88_wifi`, and `usb_host` handing out Bluetooth controllers

**From:** DRV · **To:** LANDING · **Written:** 2026-10-28 03:44 UTC · **Concerns:** 550-DRV-0042, 700-DRV-0043, 211-DRV-0040, 211-DRV-0041, 550-DRV-0036, 550-DRV-0039 · **Replaces:** LANDING/2026-10-27T1500Z-DRV-… and its correction 1505Z (not landed; this branch holds them)

**Ready:** `claude/ASR-DRV` 458735f20c47b1a96ff3e5dc34bc996e71412a06

## What it brings, besides the 1500Z request's

- **550-DRV-0042, `rtw88_wifi`:** the RTL8821CE's Wi-Fi as a `wlanhw` radio, ported from Linux `rtw88` under its BSD-3-Clause option, with its tables, host tests and `hwdocs/rtl8821ce.md`.
  - 2ca6b542 gives the beacon ring's descriptor its second segment, without which the firmware never loaded.
  - Its boot image and grant are KRN's 550-KRN-0172 (LANDING 2230Z). This branch builds without them, and the image needs them.
- **211-DRV-0041:** `rtl8168` takes XID 0x541, the DEXP's RTL8168H.
- **700-DRV-0043, `usb.wit` 1.4:**
  - `BADGE_BLUETOOTH` (5) claims a Bluetooth controller's HCI interface;
  - `usb_host` configures its endpoints and polls its bulk IN for `reports`;
  - it takes the HCI command, a class request 0 to the device, from that interface's client alone.

  The wire is unchanged, and `libmind/src/idl/usb.rs` is regenerated.

No ABI change on this branch.

## `main` in it

10ee8687, merged before 458735f2.

## What was run on the branch

- **`./02_build.sh`:** passed (2026-10-28).
- **Host tests:** `rtw88_tables`, `rtw88_driver` (8), `usb_hub`, `hid`, `serial`.
- **QEMU x86:**
  - 2026-10-28: `usb` and `keys` passed;
  - 2026-10-27: `usb`, `tablet` and `link` passed.
  - QEMU has neither the RTL8821CE nor a Bluetooth controller.
- **On the DEXP:** `rtw88_wifi` runs from a release over the cable (slot B, 2026-10-27). It joins the kept network, gets an address by DHCP, and the Effector agent connects. 700-DRV-0043 waits for `bt_hci`'s boot image.
