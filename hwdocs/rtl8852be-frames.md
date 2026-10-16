# RTL8852BE raw receive-frame boundary

Task: 213-DRV-0032; MC-2.3, MC-3.3, MC-6.1, MC-12.1, MC-12.3.

`frames.rs` decodes one complete retained PCI RX buffer into a bounded raw
802.11 MPDU and the existing `wlanhw` 1.1 eight-byte receive prefix. The
runtime owner must apply its actual station filter before delivery and keep
the admitted channel stable until queued frames are drained. Channel changes
must drain/discard the previous epoch explicitly; the descriptor does not
supply an independently verified channel. This helper does not perform tuning,
association, queue dispatch or native service activation.

## Pinned source and scope

The BSD-3-Clause choice of Linux
[`05f7e89ab9731565d8a62e3b5d1ec206485eeb0b`](https://github.com/torvalds/linux/tree/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89)
provides the AX descriptor fields and receive contract:

- `core.c:3284–3336` decodes data rate from short descriptor dword1 bits 24:16.
  `core.h:311–323` maps legacy indices 0..11 to CCK/OFDM rates. Unknown,
  reserved or MCS encodings are refused by this initial legacy-only boundary.
- `core.c:3495` marks either ICV or CRC errors as failed integrity;
  `core.c:6414` advertises `RX_INCLUDES_FCS`. The helper rejects those status
  errors and removes exactly the final four bytes. Hardware integrity status
  and DMA contents remain part of the device trust contract; this is not a
  second software CRC calculation or physical integrity guarantee.
- Hardware-decrypted frames are refused: software station crypto must receive
  the original protected MPDU. Protected frames without hardware decryption
  remain unchanged. AMSDU/cut payloads are refused in this initial scope.
- Shift, driver-info and long-descriptor offsets come from the existing checked
  RX parser. Management/data/control header minima and the 2346-byte MPDU bound
  are checked after FCS removal. Output size and channel validation precede all
  writes, and reserved prefix bytes are zero.

Signal is reported as zero, the IDL's explicit unmeasured value; per-packet
PPDU/RSSI correlation has not been implemented. The rate is in 500 kb/s units.
The scan flag reflects the actual caller's `all-beacons` filter. The helper
adds no interface version or new meaning to the existing fields.

Source SHA-256:

- `core.c`: `41ea6a22a4ff78035364def00c70edb49d821f2d4cb12869f1a0a87e6ee54886`.
- `core.h`: `48ebe2404ae8f57ff3260687580d6f867d3b2725bdf93db01ceb9ab3a8c9f138`.
- `txrx.h`: `f0e77298f5bcc26ad2b4a3f7547ae5130ac55a9309010f6261542c818c451cbb`.

## Validation boundary

Five focused host cases compile, covering the exact serialized prefix and
unchanged MPDU, all legacy rates, unsupported/error states, variable header
bounds, maximum length, descriptor offsets, truncated buffers and unchanged
output on refusal. Execution follows own-branch build and raw publication.
No actual Ryzen reception, RF integrity or WLAN connectivity is established.


Source `bd46b64f`, built with consolidated ASTRA `ac3fa1f4` and published in
raw `fast-test` `3881b629`, passed all five focused cases (0.00 s, no failures
or ignores). Independent review confirmed the pinned fields/FCS contract and
bounds. A subsequent integration check found that the retained Session inbox
keeps bytes through the validated packet end, intentionally omitting any
trailing RXBD padding. Re-running the full-buffer parser on that shortened
copy would reject a valid padded packet. `from_packet` now accepts the same
validated descriptor and retained bytes, checks all accessed fields/payload
bounds and removes only the FCS. Full-buffer `decode` remains strict. A sixth
fixture covers padded full buffers, the shortened valid copy and missing
payload bytes; its execution awaits publication of this correction.
