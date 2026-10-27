# 550-KRN-0172 — The boot image `rtw88_wifi` and init's grant for the DEXP's RTL8821CE (ABI 21)

**Type:** kernel (ABI, `init`, the build) · **Owner:** `KRN` · **Priority:** P1 · **Status:** open (made; landing and a run on the DEXP left) · **Blocked by:** — · **Main task:** [550](550-network-on-real-hardware.md) · **Roadmap:** track A (drivers), track D (network) · **Constitution:** MC-1.3, MC-1.5, MC-3.1, MC-12.1, MC-12.4

**Asked by:** the drivers track (`DRV`) on 2026-10-27, for [550-DRV-0042](550-DRV-0042-rtl8821ce-wifi.md), in `KRN/2026-10-27T1700Z-DRV-the-boot-image-rtw88-wifi-and-its-grant.md` on `internal-dev-messages`.
- **Why now:** the maintainer's order for the DEXP is Wi-Fi first. No Ethernet cable is at hand, so the RTL8821CE is the only way onto the network for the server's Effector agent.

## Problem

- **The driver is written:** `rtw88_wifi`, on `claude/ASR-DRV` 2007b0ba. It serves `idl/wlanhw.wit` 1.2 as `wlan`'s radio.
- **It cannot run without the kernel track's part,** not even through `replace`:
  - it is not a boot image;
  - `init` grants it nothing.

## Plan

1. **`common/abi.rs`:**
   - `rtw88_wifi` is the 44th boot image, appended after the prefix.
   - `ABI_VERSION` and `BOOT_HANDOFF_VERSION` become 21, and `BOOT_HANDOFF_OLDEST` becomes 21. The new image moves every field after `extra_programs`, so an older loader is refused by name.
   - Before the first release no older handoff is read (AGENTS.md, section 3). So the kernel's zeroing of `boot_flags` for a loader before handoff 20 goes, with `BOOT_FLAGS_SINCE`.
2. **`02_build.sh`:** `rtw88_wifi:rtw88_wifi:rtw88_wifi.elf`.
3. **`init`, a branch for `rtw88_wifi`:**
   - **the device:** PCI 10EC:C821, class 02:80;
   - **`SLOT_DEV0`:** BAR2 with memory decoding only (`PLATFORM_DEVICE_BAR_IDLE`). The driver turns mastering on once its rings are programmed;
   - **arguments:** the device's index;
   - **`SLOT_MEM`:** a 1 MiB DMA region. It comes from the arena, which the bootloader takes below 4 GiB, as rtw88's 32-bit DMA needs;
   - **`SLOT_VFS`:** a reader's client of `vfs_server` for `data/firmware/rtw88/`;
   - **`SLOT_SERVICE`:** its `wlanhw` endpoint, whose client goes to `wlan` alone;
   - **no interrupt:** the rings are polled;
   - **a restart:** grants the registers only (`registers_only`), as for `rtw89_wifi`;
   - **`RADIOS`:** it joins between `bcm_wifi` and `wlansim`.
4. **`init/src/services.rs`:** `rtw88_wifi` in `CONFIGURABLE`, `("wlan", "rtw88_wifi")` in `BEFORE`, and in `SAFE_LEFT_OUT`.
5. **Docs:** README's table of services, docs/profile/bootstrap.md, docs/api (ABI 21 and the handoff), docs/profile's MC-11.1 row.

## Acceptance criteria

- **Builds:** x86 and aarch64.
- **Host tests:** `tests/boot_abi_host.rs` (handoff 21 only, the 44th image, `BootInfo`'s size), `tests/runtime.rs` (ABI 21).
- **QEMU, x86:** the boot and one-CPU groups pass. On a machine without the card, init does not start `rtw88_wifi`, and `wlan` takes `wlansim`.
- **On the DEXP,** through `DRV`'s slot update by `effector485`:
  - init logs `[INIT] STARTED rtw88_wifi`;
  - the driver reads its firmware;
  - `wlan` takes it as its radio.

  The machine and the build are named (MC-12.1).

## Related

[550-DRV-0042](550-DRV-0042-rtl8821ce-wifi.md), [550-KRN-0097](../issues-done/550-KRN-0097-the-wifi-station-in-the-boot-services.done) (`wlan` and `RADIOS`), [213-KRN-0114](213-KRN-0114-realtek-driver-startup.md) (the pattern for `rtw89_wifi`), [173-KRN-0036](../issues-done/173-KRN-0036-safe-start.done) (`boot_flags`).
