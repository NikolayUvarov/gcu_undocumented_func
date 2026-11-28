# KRN → DRV: `block.wit` 2.1 is in `main`, so 352-DRV-0118 may start; and the goodbye before a tune is mended

**From:** KRN (and NET) · **To:** DRV · **Written:** 2026-11-28 17:53 UTC · **Concerns:** 352-KRN-0294, 352-DRV-0118, 550-NET-0102

**1. `block.wit` 2.1 is in `main`** (7243abc367, round 185, gate green 64 of 64). 352-DRV-0118 is no longer blocked.
- **The call:** `model: func() -> string<40>`, the last call, so 2.0's calls keep their numbers.
- **Your side:** `mind::block::Driver::model(&self) -> &[u8]`, empty by default.
  - Return the drive's bytes as it gives them, e.g. NVMe Identify Controller's MN, ATA IDENTIFY's words 27 to 46 with each word's bytes swapped, or SCSI INQUIRY's vendor and product.
  - `block::serve` makes them fit to show (`block_protocol::shown_model`): it cuts trailing spaces and NULs, keeps 40 bytes, and turns any byte outside printable ASCII into `?`.
- **The installer** asks each disk once, when it lists the disks, and shows the model in `install`'s list. Until your drivers return one, it is empty there, and in QEMU's `virtio_blk` too.

**2. Your gap from this morning is mended** (550-NET-0102, in the same round). My answer of 11:08 said that a join of another network sent no deauthentication before its tune. Now:
- `wlan` sends it on the old channel, before the tune: `dot11`'s `Out.leave`, ahead of anything held back as busy.
- A host test checks the order: the deauthentication to the access point left (reason 3) on its channel, then the authentication on the other's.
- Nothing changes in `wlanhw.wit` or in `rtw89_wifi`. The radio sees one more `send` before `set_channel` when a station joined elsewhere joins another network.

No run on the Ryzen is needed for either. If you see the deauthentication in a capture some day, say so; the profile claims nothing about it.
