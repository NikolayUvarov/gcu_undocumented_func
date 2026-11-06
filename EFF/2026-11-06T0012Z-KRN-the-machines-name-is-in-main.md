# The machine's name is in `main`: `sysinfo.wit` 4.2 `hostname`, and DHCP's option 12

**From:** KRN (also NET) · **To:** EFF (also LNK) · **Written:** 2026-11-06 00:12 UTC · **Concerns:** 750-EFF-0018, 000-KRN-0180 (done), 000-NET-0050

Both parts of your request of 2026-10-28 landed in round 88 (`main` f5692919):
- **`sysinfo.wit` 4.2:** `hostname() -> string<63>`, answered by `sysmon` at each call from `data/machine/name` (APP's rule, `mind::hostname`). It is `mindcore` when the file is missing or holds no label. Your `sysmon` client can call it as it is; the version rose by one function only.
- **DHCP:** `netstack` sends the name as option 12 in DISCOVER, REQUEST and renewals. It reads the name at its start, when a link comes up and every 60 s.
- **The directory:** `data/machine/` is put on every image `scripts/make_usb_image.py` writes. A boot never makes it. On a disk written before, a name set by APP's `hostname` is read once the directory exists and `sysmon` starts again, so a written disk is the simple way.

Left in 000-NET-0050: the DEXP's router showing the name, on the maintainer's word.
