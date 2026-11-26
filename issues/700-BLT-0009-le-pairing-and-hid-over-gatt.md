# 700-BLT-0009 — LE: SMP pairing, the ATT client, GATT discovery, HID over GATT

**Type:** library (`bt_host`) · **Owner:** `BLT` · **Priority:** P2 (after audio and classic HID, unless the maintainer's devices are LE) · **Status:** in progress (2026-11-26) · **Blocked by:** — · **Main task:** [700](700-bluetooth.md) · **Constitution:** MC-10.1, MC-11.4, MC-12.1

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

## Progress (2026-11-26)

Written in `bt_host` (`smp_crypto.rs`, `le.rs`), linked into `bt_hci`.
- **The functions** are `e`, `c1`, `s1`, `f4`, `f5`, `f6`, `g2` and `ah` on RustCrypto `aes` 0.8 and `cmac` 0.7, and the P-256 key pair and DHKey on `p256` 0.13 (as `tls/`). `bt_hci` builds them with `aes_force_soft`, as `wlan` does.
- **Random values** come from AES-128 in counter mode under a seed the driver draws from RDRAND at start (the testing stage: no reseeding). Without RDRAND, LE does not pair, and the log says so.
- **SMP as initiator.** It asks for NoInputNoOutput, bonding, Secure Connections and the peripheral's LTK and identity.
  - Secure Connections Just Works: public keys, confirm, randoms, DHKey checks, the LTK from f5.
  - Else legacy Just Works: TK 0, c1, s1, the STK, then the keys distributed.
  - Its number (g2) is logged, not compared (the testing stage).
  - A key shorter than 7 octets, a wrong confirm value or DHKey check, or a public key off the curve fails the pairing.
  - The LTK is kept (`keys.txt`'s `le` line) and encrypts the next link. A key the device refused is forgotten and the link paired again.
- **ATT and GATT.**
  - Exchange MTU (247), the HID service by Find By Type Value, its characteristics by Read By Type, the descriptors by Find Information, the report references, the report map (with Read Blob).
  - The boot protocol where the device has boot reports, else the report protocol. The input reports' notifications are turned on one write at a time.
  - Boot reports go into the classic path. In the report protocol, the map's keyboard report as a boot one. Other reports go up with the map, laid out by `mind::hid::Pointer::parse` in `bt_hci`.
  - The peer's own requests are answered as by a server with no attributes.
- **Links.** An LE connection is waited for 30 s (a device advertises only while awake) and tried again 2 s later.

**What is not done.**
- The peripheral role.
- Resolving private addresses (a device with a resolvable private address is not found again: its IRK is not kept).
- Passkey and numeric comparison with a user.
- Signing.

## Acceptance criteria

- [x] Host tests (`bt_host/tests/smp.rs`, `le.rs`):
  - RFC 4493's AES-CMAC vectors, and c1, s1, f4, f5, f6, g2 and ah against Core 5.3 Vol 3 Part H 2.2.3, 2.2.4 and Appendix D;
  - one DHKey both ways, a point off the curve refused;
  - Secure Connections and legacy Just Works against a simulated peripheral computing its own side;
  - the keys distributed and kept;
  - a later link encrypted with the key;
  - a wrong confirm value and a short key refused;
  - GATT discovery down to a boot keyboard's notification;
  - a report map's keyboard ID.
- [x] On the Ryzen: the maintainer's Logitech MX Vertical works (2026-11-26).
  - **Paired** in legacy Just Works (`ITS IO CAPABILITY 3, AUTHENTICATION 01, LEGACY PAIRING`), its LTK and identity kept.
  - **HID over GATT** in the boot protocol (`HID SERVICE 0021-0037, 8 CHARACTERISTICS, BOOT PROTOCOL, REPORT MAP 97 BYTES`, `HID READY`).
  - **Reconnected** with its key after a click woke it (`ENCRYPTING WITH ITS KEY`, `ENCRYPTED`).
  - **Moved the pointer:** 24 to 125 reports a second, every input call OK; the maintainer confirms the pointer and both buttons. *That was with the bench branch's input privilege.*

## Progress (2026-11-26): the bench

**Mended:**
- The link dropped (reason 08) 16 s into a stream. It no longer did once the source stayed central (700-BLT-0005).
- The mouse asked its parameters again every 30 s while given a 5 s timeout for its 2.16 s. It is now given what it asks, raised only below 2 s.

**Not done.**
- **Resolving private addresses:** the IRK is not kept. The mouse used its static address.
- **Our own keys:** no Identity Information is distributed by us.
- **A sleeping mouse is found again only when a click wakes it,** as a bonded LE mouse behaves.
- **Secure Connections on hardware:** the mouse paired legacy. Secure Connections is host-tested only.

## Related

[700-BLT-0005](700-BLT-0005-connections-carry-data.md), [700-BLT-0008](700-BLT-0008-classic-hid-host.md).
