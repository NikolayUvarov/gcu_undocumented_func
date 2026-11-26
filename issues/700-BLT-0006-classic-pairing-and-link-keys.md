# 700-BLT-0006 — Classic pairing: Secure Simple Pairing, PIN 0000 for old devices, link keys kept

**Type:** library (`bt_host`) and driver (`bt_hci`) · **Owner:** `BLT` · **Priority:** P1 · **Status:** in progress (2026-11-26) · **Blocked by:** `KRN`: a writable `data/bluetooth` for `bt_hci` (asked on `internal-dev-messages`, 2026-11-26), for keys kept across boots · **Main task:** [700](700-bluetooth.md) · **Constitution:** MC-10.1, MC-12.1, MC-12.3

## Problem

A headset, a keyboard or a mouse accepts profile channels only on an authenticated, encrypted link (security mode 4). Nothing in `bt_hci` paired.

## Plan

1. **On a link we made:** `Authentication Requested`, then `Set Connection Encryption`. The profile starts once `Encryption Change` says on.
2. **Secure Simple Pairing.** The controller does the ECDH.
   - **Link Key Request:** the key kept, else a negative reply.
   - **IO Capability Request:** NoInputNoOutput with general bonding (Just Works) for audio. DisplayOnly with MITM for a keyboard, which then types the passkey: the log says `TYPE nnnnnn ON IT, THEN ENTER`.
   - **User Confirmation Request:** confirmed and logged. The testing stage has nobody to compare a number with.
3. **A key refused:** a device that lost its key (status 05 or 06) is paired again once.
4. **Legacy pairing:** PIN "0000" for a device listed.
5. **Strangers:** IO capability requests and PIN requests from a device not listed are refused.
6. **The keys** go to `data/bluetooth/keys.txt` when `bt_hci` has a writable client of that directory (KRN's grant, asked). Without it they are kept for the boot only, and the start line says so. The keys never reach the log.

**What is not done (the hardening stage).**
- Keys sealed by `keystore` and the TPM (issue 700's plan, MC-10.x).
- A user's yes for each pairing, and comparing the numbers.
- Pairing allowed only while the user asks.
- The keys file is plain text on the boot volume's data area, said plainly: this is the testing stage's place for them.

## Acceptance criteria

- [x] Host tests (`bt_host/tests/links.rs`):
  - the whole SSP sequence;
  - the key kept and offered at the next link;
  - a refused key forgotten and paired again once, then the link closed;
  - DisplayOnly and MITM for a HID device and its passkey logged;
  - PIN 0000;
  - strangers refused;
  - the keys file read and written;
  - no key in the log.
- [ ] On the Ryzen: the maintainer's headphones or keyboard pair (`PAIRING DONE`, `LINK KEY OF TYPE n KEPT`, `ENCRYPTED`).
- [ ] With KRN's grant: the key survives a restart (`1 KEYS` at the next start, no pairing).

## Related

[700-BLT-0005](700-BLT-0005-connections-carry-data.md), [700-BLT-0007](700-BLT-0007-a2dp-source.md), [700-BLT-0008](700-BLT-0008-classic-hid-host.md).
