# 700-KRN-0176 — The boot image `bt_hci` and init's grant for a USB Bluetooth controller (ABI 23, handoff 23)

**Type:** kernel (ABI, `init`, the build) · **Owner:** `KRN` · **Priority:** P1 · **Status:** open (made; landing and a run on the DEXP left) · **Blocked by:** — · **Main task:** [700](700-bluetooth.md) · **Roadmap:** track A (drivers) · **Constitution:** MC-1.3, MC-1.5, MC-3.1, MC-12.1, MC-12.4

**Asked by:** the Bluetooth track (`BLT`) on 2026-10-28, for [700-BLT-0001](700-BLT-0001-bt-hci-the-rtl8821c-up.md), in `KRN/2026-10-28T0329Z-BLT-the-boot-image-bt-hci-and-its-grant.md` on `internal-dev-messages`. The maintainer's order for the DEXP: Wi-Fi, Ethernet, the screen, then Bluetooth "to connect devices".

## Problem

The DEXP's RTL8821CE has a Bluetooth half, `0BDA:C821`, on its xHCI.
- `usb_host` enumerates it, and since [700-DRV-0043](700-DRV-0043-usb-host-hands-out-bluetooth-controllers.md) hands its HCI interface to a client badged `BADGE_BLUETOOTH`.
- `bt_hci` (700-BLT-0001) resets it, downloads its patch from `data/firmware/rtl_bt/` and listens.
- No boot image carried `bt_hci`, and `init` gave it nothing.

## What was done

- **`common/abi.rs`:**
  - `bt_hci` is the 45th boot image, right after `rtw88_wifi`. That is after `usb_host`, whose client it holds, and after `vfs_server`, whose reader reads its patch;
  - **ABI 23 and handoff 23.** The image renumbers the images after it and moves every field after `extra_programs`, so a loader of handoff 21 is refused by name. A slot update carries its own loader (351-UPD-0010).
- **`02_build.sh`** builds it.
- **`init`, only when `usb_host` runs:**
  - `usb_host`'s client badged `mind::usb::BADGE_BLUETOOTH` in `SLOT_DEV0`: the wireless controller interface (class E0/01/01) and nothing else;
  - a `vfs_server` reader badged `BADGE_READER` in `SLOT_VFS`, when `vfs_server` runs;
  - no endpoint, network, display or other device;
  - without `usb_host`: `bt_hci NOT STARTED`.
- **`SAFE_LEFT_OUT`** (a radio's driver) and **`CONFIGURABLE`** name it.
  - The safe-start line now names every service left out; it printed only the first five of the list's six.
- **`HOLDS`, README and `docs/profile/bootstrap.md`** say what it holds; `docs/api` gives the new versions.

## Acceptance criteria

- **Builds:** x86 and aarch64.
- **Host tests:**
  - `tests/boot_abi_host.rs`: handoff 23, the 45th image after `rtw88_wifi` and after `usb_host` and `vfs_server`, `BootInfo`'s size;
  - `tests/runtime.rs`: ABI 23;
  - `tests/services_host.rs` and `tests/shell_host.rs`.
- **QEMU, x86:** the boot suite passes, and with no Bluetooth controller in the machine `bt_hci` holds its client and waits.
- **On the DEXP** (BLT's run, through a slot update by `effector485`):
  - `bt_hci` patches the controller;
  - it lists the devices it hears.

  The machine and the build are named (MC-12.1).

## Related

[700](700-bluetooth.md), [700-BLT-0001](700-BLT-0001-bt-hci-the-rtl8821c-up.md), [700-DRV-0043](700-DRV-0043-usb-host-hands-out-bluetooth-controllers.md), [550-KRN-0172](550-KRN-0172-rtw88-wifi-boot-image-and-grant.md) (the pattern), [800-KRN-0173](800-KRN-0173-a-firmware-scope-for-link-in-developer-mode.md) (the firmware put over the cable).
