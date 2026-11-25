# 700-BLT-0009 — LE: SMP pairing, the ATT client, GATT discovery, HID over GATT

**Type:** library (`bt_host`) · **Owner:** `BLT` · **Priority:** P2 (after audio and classic HID, unless the maintainer's devices are LE) · **Status:** open (2026-11-26) · **Blocked by:** — · **Main task:** [700](700-bluetooth.md) · **Constitution:** MC-10.1, MC-11.4, MC-12.1

## Problem

Most current mice and many keyboards are LE-only and speak HID over GATT.
- 700-BLT-0005 makes their link: `LE Create Connection` and the LE signalling channel.
- Nothing pairs or reads them. Meanwhile their ATT requests are answered "not supported" and SMP's "pairing not supported", so that they do not wait on us.

## Plan

1. **SMP (Core Vol 3 Part H).**
   - Legacy Just Works: c1 and s1 with AES-128.
   - LE Secure Connections: P-256 ECDH, and f4, f5, f6 and g2 on AES-CMAC.
   - The tree's RustCrypto `aes` and `p256` crates (as `wpa/` and `tls/` use them), checked against the specification's sample data.
   - Keys distributed are kept with the device: the LTK, EDIV and Rand. A later link starts encryption with them.
2. **The ATT client:** exchange MTU, then find, read, write, notifications and indications.
3. **GATT discovery** of the HID service (0x1812): the report map, the reports, their client configuration, and the protocol mode set to report.
4. **HID over GATT.** Notifications of input reports are laid out by the report map (`mind::hid::Pointer::parse` for a pointer) and go into the same input path as 700-BLT-0008.

## Acceptance criteria

- [ ] Host tests: SMP against the specification's sample data (c1, s1, f4, f5, f6, g2), the pairing exchanges, ATT and GATT discovery, a HOGP report.
- [ ] On the Ryzen: an LE mouse or keyboard of the maintainer's works.

## Related

[700-BLT-0005](700-BLT-0005-connections-carry-data.md), [700-BLT-0008](700-BLT-0008-classic-hid-host.md).
