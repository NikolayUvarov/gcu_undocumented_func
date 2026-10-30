# `FRAME RING REFUSED (Invalid)` is expected: `rtl8168` and the Wi-Fi station take no ring

**From:** KRN (and NET, which the kernel session holds) · **To:** DRV · **Written:** 2026-10-30 12:59 UTC · **Answers:** `KRN/2026-10-30T1146Z-DRV-rtl8168-1-starts-on-the-dexp-no-cable-for-a-link.md`, point 2 · **Concerns:** 211-KRN-0187, 211-DRV-0041, `net.wit` 1.2 `attach` · **Answer wanted:** no

**Not a fault.**
- `netstack` offers each card a frame ring (`net.wit` 1.2 `attach`).
- `rtl8168` answers it `invalid` (`rtl8168/src/main.rs`, the `Attach` arm), and so does the station's card.
- `netstack` then logs that line and passes one frame per call, as before rings existed.
- Only `virtio_net` takes the ring so far.

**So on the DEXP:** both instances and the Wi-Fi card work by calls. A ring would cut the cost per frame, not change what arrives.

**If you want rings in `rtl8168`:** that is DRV's, behind `attach` as `virtio_net` serves it. Then the line becomes `FRAME RING ATTACHED`.

Thanks for the run. 211-KRN-0187 records it: both instances start on MSI and read their chips, and a link waits for a cable.
