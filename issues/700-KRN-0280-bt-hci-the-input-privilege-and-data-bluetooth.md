# 700-KRN-0280 — `bt_hci`: the input privilege and a writable `data/bluetooth/`

**Type:** `init` (grants; no ABI change) · **Owner:** `KRN` (the kernel session) · **Priority:** P1 · **Status:** in progress (made, 2026-11-26) · **Blocked by:** — · **Main task:** [700](700-bluetooth.md) · **Constitution:** MC-3.1, MC-6.3, MC-12.3

## Problem

`BLT`'s message of 2026-11-25 (`KRN/2026-11-25T2243Z-BLT-…`) asks for two grants. It is for 700-BLT-0006 and 0008, on the maintainer's word that Bluetooth is closed while the Ryzen is at hand. `bt_hci` runs the host stack for the testing stage: links, pairing, an A2DP source and a HID host, for the devices `data/bluetooth/devices.txt` lists.
- **A Bluetooth keyboard's and mouse's reports** go through `mind::keyboard::deliver` and `mind::dev::input_key`, as `usb_hid`'s do. That needs the input privilege.
- **Link keys** are kept in `data/bluetooth/keys.txt`, the testing stage's place. That needs a writable client confined to that directory.

## Plan (made)

- **`init`'s `bt_hci` arm:** `SLOT_PRIV` gets `minted.privilege(CAP_KIND_INPUT)`, the line `usb_hid` has.
- **`scopes_of`:** `bt_hci` gets `(SLOT_FILE, BADGE_USER, "data/bluetooth", writable, made)`. Its firmware reader in `SLOT_VFS` stays as it was.
- **The profile:**
  - `bootstrap.md`'s `bt_hci` row;
  - in `tcb.md` and `aarch64/tcb.md`, `bt_hci` joins those that can inject input.
- **Not here:** BLT's third line (`KRN/2026-11-25T2337Z-BLT-…`), an `audio_gw` client badged `BADGE_SINK`. That badge comes with `audio.wit` 1.3 (700-DRV-0110), not yet in `main`; the line follows when it lands.

**The hardening stage, not done** (BLT's issues name it):
- a `bt_host` service with its own capabilities, so the input privilege leaves the driver;
- link keys sealed by `keystore`;
- a pairing policy.

## Acceptance criteria

- `init` builds on both architectures. QEMU's `services` and `bluetooth` checks pass with `bt_hci` started where they start it.
- On the Ryzen or the DEXP (BLT's run): `bt_hci`'s start line says `HID INPUT DELIVERED`, and `keys.txt` is written in `data/bluetooth/`, for that machine and build only (MC-12.1).

## Related

[700](700-bluetooth.md), [700-KRN-0176](700-KRN-0176-bt-hci-boot-image-and-grant.md), BLT's 700-BLT-0006 and 0008 (on `claude/700-BLT-0005-bt-connections`).
